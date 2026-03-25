//! Build script: generates static block state and tag data from vanilla JSON.
//!
//! Reads compressed block JSON, parses all block types and states,
//! computes property strides for O(1) state transitions, and writes
//! `block_states_generated.rs` into `$OUT_DIR/`.
//!
//! Also reads `tags.json` (vanilla tags) and custom Oxidized tag files,
//! resolving block names to type IDs, and writes `block_tags_generated.rs`.

// Build scripts run at compile time; expect/panic are the standard error
// reporting mechanism and are not reachable at runtime.
#![allow(clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Read;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let manifest_path = Path::new(&manifest_dir);
    let data_path = manifest_path.join("src/data/blocks.json.gz");
    let props_path = manifest_path.join("src/data/block_properties.json.gz");

    // Vanilla tags from the protocol crate
    let tags_path = manifest_path
        .parent()
        .expect("parent of oxidized-world")
        .join("oxidized-protocol/src/data/tags.json");

    // Custom Oxidized tag directory
    let custom_tags_dir = manifest_path.join("src/data/tags/block");

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed={}", data_path.display());
    println!("cargo::rerun-if-changed={}", props_path.display());
    println!("cargo::rerun-if-changed={}", tags_path.display());
    println!("cargo::rerun-if-changed={}", custom_tags_dir.display());

    let compressed = fs::read(&data_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", data_path.display()));
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut json_str = String::new();
    decoder
        .read_to_string(&mut json_str)
        .expect("Failed to decompress blocks.json.gz");

    let root: serde_json::Value = serde_json::from_str(&json_str).expect("Invalid blocks.json");
    let obj = root
        .as_object()
        .expect("blocks.json root must be an object");

    // ── Load block properties ───────────────────────────────────────────
    let block_props = load_block_properties(&props_path);

    // ── Parse all blocks ────────────────────────────────────────────────
    let mut blocks: Vec<BlockData> = Vec::with_capacity(obj.len());
    for (name, value) in obj {
        blocks.push(parse_block(name, value));
    }

    // Sort by first state ID → vanilla registration order
    blocks.sort_by_key(|b| b.first_state_id);
    for (i, block) in blocks.iter_mut().enumerate() {
        block.type_index = i as u16;
    }

    let state_count = blocks
        .iter()
        .flat_map(|b| &b.states)
        .map(|s| s.id as usize + 1)
        .max()
        .unwrap_or(0);

    // ── Build per-state data ────────────────────────────────────────────
    let mut state_block_type = vec![0u16; state_count];
    let mut state_flags = vec![0u16; state_count];
    let mut state_light_emission = vec![0u8; state_count];
    let mut state_light_opacity = vec![0u8; state_count];
    let mut state_hardness = vec![0u16; state_count];
    let mut state_explosion_resistance = vec![0u16; state_count];
    let mut state_friction = vec![0u16; state_count];
    let mut state_speed_factor = vec![0u16; state_count];
    let mut state_jump_factor = vec![0u16; state_count];
    let mut state_map_color = vec![0u8; state_count];
    let mut state_push_reaction = vec![0u8; state_count];

    for block in &blocks {
        let base_flags = compute_flags(&block.name, &block.definition_type, &block_props);
        let props = block_props
            .get(&block.name)
            .cloned()
            .unwrap_or_else(|| BlockProperties {
                has_collision: true,
                is_air: false,
                is_liquid: false,
                is_replaceable: false,
                is_opaque: true,
                is_flammable: false,
                requires_tool: false,
                ticks_randomly: false,
                has_block_entity: false,
                is_interactable: false,
                is_solid: false,
                light_emission: 0,
                light_opacity: 0,
                hardness: 0,
                explosion_resistance: 0,
                friction: 6000, // 0.6 * 10000
                speed_factor: 10000, // 1.0 * 10000
                jump_factor: 10000, // 1.0 * 10000
                map_color: 0,
                push_reaction: 0,
            });

        for state in &block.states {
            let idx = state.id as usize;
            state_block_type[idx] = block.type_index;
            state_flags[idx] = base_flags | if state.is_default { 0x0002 } else { 0 };
            state_light_emission[idx] = props.light_emission;
            state_light_opacity[idx] = props.light_opacity;
            state_hardness[idx] = props.hardness;
            state_explosion_resistance[idx] = props.explosion_resistance;
            state_friction[idx] = props.friction;
            state_speed_factor[idx] = props.speed_factor;
            state_jump_factor[idx] = props.jump_factor;
            state_map_color[idx] = props.map_color;
            state_push_reaction[idx] = props.push_reaction;
        }
    }

    // ── Build property tables ───────────────────────────────────────────
    let mut prop_defs: Vec<PropDefOut> = Vec::new();
    let mut prop_values: Vec<String> = Vec::new();

    for block in &mut blocks {
        block.props_offset = prop_defs.len() as u16;
        let strides = compute_strides(block);
        for (i, prop) in block.properties.iter().enumerate() {
            let values_offset = prop_values.len() as u16;
            prop_values.extend(prop.values.iter().cloned());
            prop_defs.push(PropDefOut {
                name: prop.name.clone(),
                num_values: prop.values.len() as u8,
                values_offset,
                stride: strides[i],
            });
        }
    }

    // ── Verify stride computation matches actual state IDs ──────────────
    for block in &blocks {
        let props_start = block.props_offset as usize;
        let strides: Vec<u16> = prop_defs[props_start..props_start + block.properties.len()]
            .iter()
            .map(|p| p.stride)
            .collect();
        for state in &block.states {
            let expected_offset: u16 = state
                .value_indices
                .iter()
                .zip(&strides)
                .map(|(&vi, &s)| vi as u16 * s)
                .sum();
            let expected_id = block.first_state_id + expected_offset;
            assert_eq!(
                state.id, expected_id,
                "State ID mismatch for {}: expected {} (first={} + offset={}), got {}",
                block.name, expected_id, block.first_state_id, expected_offset, state.id,
            );
        }
    }

    // ── Sorted name lookup ──────────────────────────────────────────────
    let mut name_lookup: Vec<(&str, u16)> = blocks
        .iter()
        .map(|b| (b.name.as_str(), b.type_index))
        .collect();
    name_lookup.sort_by_key(|&(n, _)| n);

    // ── Generate code ───────────────────────────────────────────────────
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest = Path::new(&out_dir).join("block_states_generated.rs");
    let mut code = String::with_capacity(4 * 1024 * 1024);

    write_generated(
        &mut code,
        &blocks,
        state_count,
        &state_block_type,
        &state_flags,
        &state_light_emission,
        &state_light_opacity,
        &state_hardness,
        &state_explosion_resistance,
        &state_friction,
        &state_speed_factor,
        &state_jump_factor,
        &state_map_color,
        &state_push_reaction,
        &prop_defs,
        &prop_values,
        &name_lookup,
    );

    fs::write(&dest, &code).expect("Failed to write generated code");

    // ── Generate block tags ─────────────────────────────────────────────
    let tag_dest = Path::new(&out_dir).join("block_tags_generated.rs");
    let tag_code = generate_block_tags(&tags_path, &custom_tags_dir, &name_lookup);
    fs::write(&tag_dest, &tag_code).expect("Failed to write block tags generated code");
}

// ─── Data structures ────────────────────────────────────────────────────────

struct BlockData {
    name: String,
    definition_type: String,
    properties: Vec<PropData>,
    states: Vec<StateData>,
    first_state_id: u16,
    default_state_id: u16,
    type_index: u16,
    props_offset: u16,
}

struct PropData {
    name: String,
    values: Vec<String>,
}

struct StateData {
    id: u16,
    is_default: bool,
    /// Property values in definition order (indices into parent block's property value lists).
    value_indices: Vec<u8>,
}

struct PropDefOut {
    name: String,
    num_values: u8,
    values_offset: u16,
    stride: u16,
}

// ─── Parsing ────────────────────────────────────────────────────────────────

fn parse_block(name: &str, value: &serde_json::Value) -> BlockData {
    let definition_type = value
        .get("definition")
        .and_then(|d| d.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("minecraft:block")
        .to_owned();

    // Properties in JSON definition order (preserve_order feature keeps insertion order).
    let properties: Vec<PropData> = value
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(k, v)| PropData {
                    name: k.clone(),
                    values: v
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let states: Vec<StateData> = value
        .get("states")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let id = s.get("id")?.as_u64()? as u16;
                    let is_default = s.get("default").and_then(|d| d.as_bool()).unwrap_or(false);

                    // Resolve value indices by matching state property values to block property
                    // value lists, in definition order.
                    let state_props = s.get("properties").and_then(|p| p.as_object());
                    let value_indices: Vec<u8> = properties
                        .iter()
                        .map(|prop| {
                            let val = state_props
                                .and_then(|sp| sp.get(&prop.name))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            prop.values.iter().position(|v| v == val).unwrap_or(0) as u8
                        })
                        .collect();

                    Some(StateData {
                        id,
                        is_default,
                        value_indices,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let first_state_id = states.iter().map(|s| s.id).min().unwrap_or(0);
    let default_state_id = states
        .iter()
        .find(|s| s.is_default)
        .map(|s| s.id)
        .unwrap_or(first_state_id);

    BlockData {
        name: name.to_owned(),
        definition_type,
        properties,
        states,
        first_state_id,
        default_state_id,
        type_index: 0,
        props_offset: 0,
    }
}

fn compute_flags(
    block_name: &str,
    definition_type: &str,
    block_props: &HashMap<String, BlockProperties>,
) -> u16 {
    // IS_AIR and IS_LIQUID from definition_type (fallback)
    let mut f = 0u16;
    if definition_type == "minecraft:air" {
        f |= 0x0001; // IS_AIR
    }
    if definition_type == "minecraft:liquid" {
        f |= 0x0004; // IS_LIQUID
    }

    // Enrich from extracted block properties
    if let Some(props) = block_props.get(block_name) {
        if props.is_air {
            f |= 0x0001; // IS_AIR
        }
        if props.is_liquid {
            f |= 0x0004; // IS_LIQUID
        }
        if props.is_solid {
            f |= 0x0008; // IS_SOLID
        }
        if props.has_collision {
            f |= 0x0010; // HAS_COLLISION
        }
        if props.is_opaque {
            f |= 0x0020; // IS_OPAQUE
        }
        if props.is_replaceable {
            f |= 0x0040; // IS_REPLACEABLE
        }
        if props.has_block_entity {
            f |= 0x0080; // HAS_BLOCK_ENTITY
        }
        if props.ticks_randomly {
            f |= 0x0100; // TICKS_RANDOMLY
        }
        if props.requires_tool {
            f |= 0x0200; // REQUIRES_TOOL
        }
        if props.is_flammable {
            f |= 0x0400; // IS_FLAMMABLE
        }
        if props.is_interactable {
            f |= 0x0800; // IS_INTERACTABLE
        }
    }
    f
}

#[derive(Debug)]
#[derive(Clone)]
struct BlockProperties {
    has_collision: bool,
    is_air: bool,
    is_liquid: bool,
    is_replaceable: bool,
    is_opaque: bool,
    is_flammable: bool,
    requires_tool: bool,
    ticks_randomly: bool,
    has_block_entity: bool,
    is_interactable: bool,
    is_solid: bool,
    light_emission: u8,
    light_opacity: u8,
    hardness: u16,
    explosion_resistance: u16,
    friction: u16,
    speed_factor: u16,
    jump_factor: u16,
    map_color: u8,
    push_reaction: u8,
}

fn load_block_properties(path: &Path) -> HashMap<String, BlockProperties> {
    let compressed =
        fs::read(path).unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut json_str = String::new();
    decoder
        .read_to_string(&mut json_str)
        .expect("Failed to decompress block_properties.json.gz");

    let root: serde_json::Value =
        serde_json::from_str(&json_str).expect("Invalid block_properties.json");
    let obj = root
        .as_object()
        .expect("block_properties.json root must be an object");

    let mut result = HashMap::with_capacity(obj.len());
    for (name, value) in obj {
        let hardness_raw = value["hardness"].as_f64().unwrap_or(0.0);
        let hardness = if hardness_raw < 0.0 {
            0xFFFF // Unbreakable (bedrock)
        } else {
            (hardness_raw * 100.0).round().min(0xFFFE as f64) as u16
        };

        let bp = BlockProperties {
            has_collision: value["has_collision"].as_bool().unwrap_or(true),
            is_air: value["is_air"].as_bool().unwrap_or(false),
            is_liquid: value["is_liquid"].as_bool().unwrap_or(false),
            is_replaceable: value["is_replaceable"].as_bool().unwrap_or(false),
            is_opaque: value["is_opaque"].as_bool().unwrap_or(true),
            is_flammable: value["is_flammable"].as_bool().unwrap_or(false),
            requires_tool: value["requires_tool"].as_bool().unwrap_or(false),
            ticks_randomly: value["ticks_randomly"].as_bool().unwrap_or(false),
            has_block_entity: value["has_block_entity"].as_bool().unwrap_or(false),
            is_interactable: value["is_interactable"].as_bool().unwrap_or(false),
            is_solid: value["is_solid"].as_bool().unwrap_or(false),
            light_emission: (value["light_emission"].as_u64().unwrap_or(0) & 0xF) as u8,
            light_opacity: (value["light_opacity"].as_u64().unwrap_or(0) & 0xF) as u8,
            hardness,
            explosion_resistance: ((value["explosion_resistance"].as_f64().unwrap_or(0.0) * 100.0)
                .round()
                .min(0xFFFF as f64)) as u16,
            friction: ((value["friction"].as_f64().unwrap_or(0.6) * 10_000.0)
                .round()
                .min(0xFFFF as f64)) as u16,
            speed_factor: ((value["speed_factor"].as_f64().unwrap_or(1.0) * 10_000.0)
                .round()
                .min(0xFFFF as f64)) as u16,
            jump_factor: ((value["jump_factor"].as_f64().unwrap_or(1.0) * 10_000.0)
                .round()
                .min(0xFFFF as f64)) as u16,
            map_color: (value["map_color"].as_u64().unwrap_or(0) & 0x3F) as u8,
            push_reaction: (value["push_reaction"].as_u64().unwrap_or(0) & 0x3) as u8,
        };
        result.insert(name.clone(), bp);
    }
    result
}

/// Compute the stride for each property by examining actual state data.
///
/// The stride of a property is the number of consecutive states (by ascending
/// ID) before that property's value first changes from its initial value.
fn compute_strides(block: &BlockData) -> Vec<u16> {
    let n = block.properties.len();
    if n == 0 {
        return Vec::new();
    }

    // States sorted by ID (should already be, but be safe).
    let mut sorted: Vec<&StateData> = block.states.iter().collect();
    sorted.sort_by_key(|s| s.id);

    let mut strides = vec![0u16; n];
    for (prop_idx, _) in block.properties.iter().enumerate() {
        let first_val = sorted[0].value_indices[prop_idx];
        let mut found = false;
        for (offset, state) in sorted.iter().enumerate().skip(1) {
            if state.value_indices[prop_idx] != first_val {
                strides[prop_idx] = offset as u16;
                found = true;
                break;
            }
        }
        if !found {
            // Property has only one distinct value → stride is total states.
            strides[prop_idx] = block.states.len() as u16;
        }
    }

    strides
}

// ─── Code generation ────────────────────────────────────────────────────────

fn write_generated(
    out: &mut String,
    blocks: &[BlockData],
    state_count: usize,
    state_block_type: &[u16],
    state_flags: &[u16],
    state_light_emission: &[u8],
    state_light_opacity: &[u8],
    state_hardness: &[u16],
    state_explosion_resistance: &[u16],
    state_friction: &[u16],
    state_speed_factor: &[u16],
    state_jump_factor: &[u16],
    state_map_color: &[u8],
    state_push_reaction: &[u8],
    prop_defs: &[PropDefOut],
    prop_values: &[String],
    name_lookup: &[(&str, u16)],
) {
    let _ = writeln!(out, "// @generated by build.rs — do not edit\n");
    let _ = writeln!(out, "/// Total number of block types.");
    let _ = writeln!(out, "pub const BLOCK_COUNT: usize = {};\n", blocks.len());
    let _ = writeln!(out, "/// Total number of block states.");
    let _ = writeln!(out, "pub const STATE_COUNT: usize = {state_count};\n");

    // ── BLOCK_STATE_DATA ────────────────────────────────────────────────
    let _ = writeln!(
        out,
        "/// Block state data indexed by state ID (dense, no gaps)."
    );
    let _ = writeln!(
        out,
        "pub static BLOCK_STATE_DATA: [BlockStateEntry; STATE_COUNT] = ["
    );
    for i in 0..state_count {
        let _ = writeln!(
            out,
            "    BlockStateEntry {{ block_type: {}, flags: BlockStateFlags::from_bits_truncate({}), \
             light_emission: {}, light_opacity: {}, hardness: {}, explosion_resistance: {}, \
             friction: {}, speed_factor: {}, jump_factor: {}, map_color: {}, push_reaction: {} }},",
            state_block_type[i], state_flags[i],
            state_light_emission[i], state_light_opacity[i],
            state_hardness[i], state_explosion_resistance[i],
            state_friction[i], state_speed_factor[i], state_jump_factor[i],
            state_map_color[i], state_push_reaction[i],
        );
    }
    let _ = writeln!(out, "];\n");

    // ── BLOCK_DEFS ──────────────────────────────────────────────────────
    let _ = writeln!(out, "/// Block definitions indexed by block type index.");
    let _ = writeln!(out, "pub static BLOCK_DEFS: [BlockDef; BLOCK_COUNT] = [");
    for b in blocks {
        let _ = writeln!(
            out,
            "    BlockDef {{ name: {:?}, first_state: {}, state_count: {}, default_state: {}, \
             prop_count: {}, props_offset: {} }},",
            b.name,
            b.first_state_id,
            b.states.len(),
            b.default_state_id,
            b.properties.len(),
            b.props_offset,
        );
    }
    let _ = writeln!(out, "];\n");

    // ── PROPERTY_DEFS ───────────────────────────────────────────────────
    let _ = writeln!(
        out,
        "/// Property definitions (flat array, referenced by BlockDef)."
    );
    let _ = writeln!(
        out,
        "pub static PROPERTY_DEFS: [PropertyDef; {}] = [",
        prop_defs.len()
    );
    for p in prop_defs {
        let _ = writeln!(
            out,
            "    PropertyDef {{ name: {:?}, num_values: {}, values_offset: {}, stride: {} }},",
            p.name, p.num_values, p.values_offset, p.stride,
        );
    }
    let _ = writeln!(out, "];\n");

    // ── PROPERTY_VALUES ─────────────────────────────────────────────────
    let _ = writeln!(
        out,
        "/// Property value strings (flat array, referenced by PropertyDef)."
    );
    let _ = writeln!(
        out,
        "pub static PROPERTY_VALUES: [&str; {}] = [",
        prop_values.len()
    );
    for v in prop_values {
        let _ = writeln!(out, "    {:?},", v);
    }
    let _ = writeln!(out, "];\n");

    // ── BLOCK_NAMES_SORTED ──────────────────────────────────────────────
    let _ = writeln!(
        out,
        "/// Block names sorted alphabetically for binary search lookup."
    );
    let _ = writeln!(
        out,
        "pub static BLOCK_NAMES_SORTED: [(&str, u16); BLOCK_COUNT] = ["
    );
    for &(name, idx) in name_lookup {
        let _ = writeln!(out, "    ({:?}, {idx}),", name);
    }
    let _ = writeln!(out, "];\n");

    // ── Well-known constants ────────────────────────────────────────────
    let _ = writeln!(out, "// Well-known block state constants (default states).");
    let well_known: HashMap<&str, &str> = [
        ("minecraft:air", "AIR"),
        ("minecraft:stone", "STONE"),
        ("minecraft:granite", "GRANITE"),
        ("minecraft:diorite", "DIORITE"),
        ("minecraft:andesite", "ANDESITE"),
        ("minecraft:grass_block", "GRASS_BLOCK"),
        ("minecraft:dirt", "DIRT"),
        ("minecraft:cobblestone", "COBBLESTONE"),
        ("minecraft:oak_planks", "OAK_PLANKS"),
        ("minecraft:bedrock", "BEDROCK"),
        ("minecraft:water", "WATER"),
        ("minecraft:lava", "LAVA"),
        ("minecraft:sand", "SAND"),
        ("minecraft:gravel", "GRAVEL"),
        ("minecraft:oak_log", "OAK_LOG"),
        ("minecraft:oak_leaves", "OAK_LEAVES"),
        ("minecraft:glass", "GLASS"),
        ("minecraft:iron_block", "IRON_BLOCK"),
        ("minecraft:gold_block", "GOLD_BLOCK"),
        ("minecraft:obsidian", "OBSIDIAN"),
        ("minecraft:torch", "TORCH"),
        ("minecraft:spawner", "SPAWNER"),
        ("minecraft:oak_stairs", "OAK_STAIRS"),
        ("minecraft:chest", "CHEST"),
        ("minecraft:diamond_ore", "DIAMOND_ORE"),
        ("minecraft:diamond_block", "DIAMOND_BLOCK"),
        ("minecraft:crafting_table", "CRAFTING_TABLE"),
        ("minecraft:furnace", "FURNACE"),
        ("minecraft:redstone_wire", "REDSTONE_WIRE"),
        ("minecraft:oak_door", "OAK_DOOR"),
        ("minecraft:ladder", "LADDER"),
        ("minecraft:rail", "RAIL"),
        ("minecraft:lever", "LEVER"),
        ("minecraft:stone_pressure_plate", "STONE_PRESSURE_PLATE"),
        ("minecraft:redstone_torch", "REDSTONE_TORCH"),
        ("minecraft:stone_button", "STONE_BUTTON"),
        ("minecraft:ice", "ICE"),
        ("minecraft:snow_block", "SNOW_BLOCK"),
        ("minecraft:cactus", "CACTUS"),
        ("minecraft:netherrack", "NETHERRACK"),
        ("minecraft:glowstone", "GLOWSTONE"),
        ("minecraft:end_stone", "END_STONE"),
        ("minecraft:emerald_block", "EMERALD_BLOCK"),
        ("minecraft:command_block", "COMMAND_BLOCK"),
        ("minecraft:barrier", "BARRIER"),
        ("minecraft:iron_trapdoor", "IRON_TRAPDOOR"),
        ("minecraft:cobweb", "COBWEB"),
        ("minecraft:soul_sand", "SOUL_SAND"),
        ("minecraft:slime_block", "SLIME_BLOCK"),
        ("minecraft:packed_ice", "PACKED_ICE"),
        ("minecraft:frosted_ice", "FROSTED_ICE"),
        ("minecraft:blue_ice", "BLUE_ICE"),
        ("minecraft:bubble_column", "BUBBLE_COLUMN"),
        ("minecraft:sweet_berry_bush", "SWEET_BERRY_BUSH"),
        ("minecraft:honey_block", "HONEY_BLOCK"),
        ("minecraft:powder_snow", "POWDER_SNOW"),
    ]
    .into_iter()
    .collect();

    for b in blocks {
        if let Some(const_name) = well_known.get(b.name.as_str()) {
            let _ = writeln!(out, "/// `{}` default state.", b.name);
            let _ = writeln!(
                out,
                "pub const {const_name}: BlockStateId = BlockStateId({});",
                b.default_state_id,
            );
        }
    }
}

// ─── Block tag generation ───────────────────────────────────────────────────

/// Generates `block_tags_generated.rs` containing static tag lookup tables.
///
/// Reads vanilla tags from `tags.json` and custom Oxidized tags from
/// `src/data/tags/block/*.json`, producing three static arrays:
/// - `TAG_NAMES` — sorted tag names for binary search
/// - `TAG_RANGES` — (start, len) pairs into `TAG_MEMBERS`
/// - `TAG_MEMBERS` — flat array of sorted block type IDs
fn generate_block_tags(
    tags_path: &Path,
    custom_tags_dir: &Path,
    name_lookup: &[(&str, u16)],
) -> String {
    // Build name → type_id map for resolving custom tag block names
    let name_to_type_id: HashMap<&str, u16> = name_lookup.iter().copied().collect();

    // ── Load vanilla block tags ─────────────────────────────────────────
    let tags_json = fs::read_to_string(tags_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", tags_path.display()));
    let tags_root: serde_json::Value =
        serde_json::from_str(&tags_json).expect("Invalid tags.json");
    let block_tags = tags_root
        .get("minecraft:block")
        .and_then(|v| v.as_object())
        .expect("tags.json must have a 'minecraft:block' key");

    let mut all_tags: Vec<(String, Vec<u16>)> = Vec::new();

    for (tag_name, ids_value) in block_tags {
        let ids: Vec<u16> = ids_value
            .as_array()
            .expect("tag entries must be arrays")
            .iter()
            .map(|v| {
                v.as_u64()
                    .unwrap_or_else(|| panic!("tag {tag_name} has non-integer entry"))
                    as u16
            })
            .collect();
        all_tags.push((tag_name.clone(), ids));
    }

    // ── Load custom Oxidized tags ───────────────────────────────────────
    if custom_tags_dir.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(custom_tags_dir)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", custom_tags_dir.display()))
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json")
            })
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            let stem = path
                .file_stem()
                .expect("file should have a stem")
                .to_str()
                .expect("file stem should be UTF-8");
            let tag_name = format!("oxidized:{stem}");

            let json_str = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
            let tag_data: serde_json::Value =
                serde_json::from_str(&json_str).unwrap_or_else(|e| {
                    panic!("Invalid JSON in {}: {e}", path.display());
                });
            let values = tag_data
                .get("values")
                .and_then(|v| v.as_array())
                .unwrap_or_else(|| panic!("{} must have a 'values' array", path.display()));

            let ids: Vec<u16> = values
                .iter()
                .map(|v| {
                    let block_name = v.as_str().unwrap_or_else(|| {
                        panic!("{}: entries must be strings", path.display())
                    });
                    *name_to_type_id.get(block_name).unwrap_or_else(|| {
                        panic!(
                            "{}: unknown block name {:?}",
                            path.display(),
                            block_name
                        )
                    })
                })
                .collect();

            all_tags.push((tag_name, ids));
        }
    }

    // ── Sort tags by name, sort members within each tag ─────────────────
    all_tags.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, members) in &mut all_tags {
        members.sort();
        members.dedup();
    }

    // ── Build flat member array ─────────────────────────────────────────
    let mut flat_members: Vec<u16> = Vec::new();
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for (_, members) in &all_tags {
        let start = flat_members.len() as u32;
        let len = members.len() as u32;
        ranges.push((start, len));
        flat_members.extend(members);
    }

    // ── Generate Rust code ──────────────────────────────────────────────
    let mut out = String::with_capacity(128 * 1024);

    let _ = writeln!(out, "// @generated by build.rs — do not edit\n");

    // TAG_NAMES
    let _ = writeln!(
        out,
        "/// Sorted tag names for binary search lookup."
    );
    let _ = writeln!(
        out,
        "pub static TAG_NAMES: [&str; {}] = [",
        all_tags.len()
    );
    for (name, _) in &all_tags {
        let _ = writeln!(out, "    {name:?},");
    }
    let _ = writeln!(out, "];\n");

    // TAG_RANGES
    let _ = writeln!(
        out,
        "/// (start, len) pairs into TAG_MEMBERS for each tag."
    );
    let _ = writeln!(
        out,
        "pub static TAG_RANGES: [(u32, u32); {}] = [",
        ranges.len()
    );
    for (start, len) in &ranges {
        let _ = writeln!(out, "    ({start}, {len}),");
    }
    let _ = writeln!(out, "];\n");

    // TAG_MEMBERS
    let _ = writeln!(
        out,
        "/// Flat array of sorted block type IDs for all tags."
    );
    let _ = writeln!(
        out,
        "pub static TAG_MEMBERS: [u16; {}] = [",
        flat_members.len()
    );
    // Write members in chunks for readability
    for chunk in flat_members.chunks(16) {
        let _ = write!(out, "    ");
        for (i, id) in chunk.iter().enumerate() {
            if i > 0 {
                let _ = write!(out, ", ");
            }
            let _ = write!(out, "{id}");
        }
        let _ = writeln!(out, ",");
    }
    let _ = writeln!(out, "];");

    out
}
