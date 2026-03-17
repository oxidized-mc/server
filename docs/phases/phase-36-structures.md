# Phase 36 — World Structures

**Crate:** `oxidized-world` + `oxidized-game`  
**Reward:** Villages, dungeons, mineshafts, and strongholds generate correctly
in newly created worlds; the `/locate` command finds them.

**Depends on:** Phase 26 (noise worldgen), Phase 23 (flat worldgen), Phase 8
(block/item registry), Phase 30 (block entities — spawners/chests), Phase 34
(loot tables — chest contents)

---

## Goal

Implement the structure generation pipeline: placement checks (where to
attempt), start piece selection, recursive piece assembly (Jigsaw BFS for
villages), and template `.nbt` loading. Structures must be fully consistent with
vanilla-generated worlds so that existing save files with structures load
correctly.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Structure placement | `StructurePlacement` | `net.minecraft.world.level.levelgen.structure.placement.StructurePlacement` |
| Random spread placement | `RandomSpreadStructurePlacement` | `...structure.placement.RandomSpreadStructurePlacement` |
| Concentric rings | `ConcentricRingsStructurePlacement` | `...structure.placement.ConcentricRingsStructurePlacement` |
| Structure set | `StructureSet` | `net.minecraft.world.level.levelgen.structure.StructureSet` |
| Structure base | `Structure` | `net.minecraft.world.level.levelgen.structure.Structure` |
| Jigsaw structure | `JigsawStructure` | `...structure.structures.JigsawStructure` |
| Jigsaw placement | `JigsawPlacement` | `net.minecraft.world.level.levelgen.structure.pools.JigsawPlacement` |
| Template pool | `StructureTemplatePool` | `net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool` |
| Structure template | `StructureTemplate` | `net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate` |
| Structure manager | `StructureManager` | `net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager` |
| Monster room | `MonsterRoomStructure` | `...structure.structures.MonsterRoomStructure` |
| Mineshaft | `MineshaftStructure` | `...structure.structures.MineshaftStructure` |
| Stronghold | `StrongholdStructure` | `...structure.structures.StrongholdStructure` |
| Structure check | `StructureCheck` | `net.minecraft.world.level.levelgen.structure.StructureCheck` |

---

## Tasks

### 36.1 — Structure placement system

#### 36.1.1 — `RandomSpreadStructurePlacement`

Controls where a structure *may* attempt to generate using a grid-based
approach. For each region `(region_x, region_z)` of size `spacing`:

```
attempt_x = region_x * spacing + rng(0, spacing - separation)
attempt_z = region_z * spacing + rng(0, spacing - separation)
```

The `rng` is seeded from `world_seed + salt + region_x * 341873128712 + region_z * 132897987541`.

```rust
// crates/oxidized-world/src/structure/placement.rs

pub struct RandomSpreadStructurePlacement {
    pub spacing: i32,
    pub separation: i32,
    pub spread_type: RandomSpreadType,
    pub salt: u64,
    pub locate_offset: (i32, i32),
    pub frequency: f32,
    pub frequency_reduction_method: FrequencyReductionMethod,
}

impl RandomSpreadStructurePlacement {
    pub fn potential_structure_chunk(&self, seed: i64, region_x: i32, region_z: i32)
        -> (i32, i32)
    {
        let mut rng = LegacyRandom::with_seed(
            seed
            .wrapping_add(self.salt as i64)
            .wrapping_add(region_x as i64 * 341873128712)
            .wrapping_add(region_z as i64 * 132897987541)
        );
        let offset_max = self.spacing - self.separation;
        let chunk_x = region_x * self.spacing + rng.next_int_bounded(offset_max);
        let chunk_z = region_z * self.spacing + rng.next_int_bounded(offset_max);
        (chunk_x + self.locate_offset.0, chunk_z + self.locate_offset.1)
    }
}
```

#### 36.1.2 — `ConcentricRingsStructurePlacement`

Used for strongholds only. Places `count` structures in rings at fixed distances
from world origin, evenly angularly spaced with a slight random jitter.

#### 36.1.3 — Structure sets (data pack)

Structure sets are loaded from `data/minecraft/worldgen/structure_set/`.
Each file lists `(structure, weight)` pairs and their `placement`.

Key sets and their parameters:

| Set | Spacing | Separation | Salt |
|-----|---------|-----------|------|
| `village` | 34 | 8 | 10387312 |
| `stronghold` | ConcentricRings: count=128, distance=32..112 | — | — |
| `dungeon` | 8 | 4 | 14357617 |
| `mineshaft` | 1 | 0 | 0 (per-chunk random) | 
| `ocean_monument` | 32 | 5 | 10387313 |
| `woodland_mansion` | 80 | 20 | 10387319 |
| `nether_fortress` | 27 | 4 | 30084232 |
| `bastion_remnant` | 27 | 4 | 30084232 |
| `end_city` | 20 | 11 | 10387313 |

### 36.2 — `StructureTemplate` (NBT loading)

Structure templates are stored as `.nbt` files and describe a bounding box of
blocks and entities to place.

```rust
// crates/oxidized-world/src/structure/template.rs

pub struct StructureTemplate {
    pub size: BlockPos,                 // (x, y, z) dimensions
    pub palette: Vec<BlockState>,       // block state palette
    pub blocks: Vec<TemplateBlock>,     // each block references palette index
    pub entities: Vec<TemplateEntity>,  // entities with relative positions + NBT
    pub data_version: i32,
}

pub struct TemplateBlock {
    pub pos: BlockPos,             // relative position within template
    pub state_index: u32,          // index into palette
    pub nbt: Option<NbtCompound>,  // block entity data (if any)
}
```

Placement supports all four cardinal rotations and two mirror axes. Rotation
matrices are applied to all block positions and also to directional block
properties (`facing`, `half`, etc.).

```rust
impl StructureTemplate {
    pub fn place(
        &self,
        level: &mut ServerLevel,
        origin: BlockPos,
        rotation: Rotation,
        mirror: Mirror,
        rng: &mut impl RngCore,
    ) {
        let transform = StructureTransform { origin, rotation, mirror };
        for tb in &self.blocks {
            let world_pos = transform.apply(tb.pos);
            let state = self.palette[tb.state_index as usize]
                .rotate(rotation)
                .mirror(mirror);
            level.set_block(world_pos, state, BlockFlags::UPDATE_CLIENTS);
            if let Some(nbt) = &tb.nbt {
                if let Some(be) = level.get_block_entity_mut(world_pos) {
                    be.load(nbt);
                }
            }
        }
    }
}
```

### 36.3 — `StructureTemplateManager`

```rust
// crates/oxidized-world/src/structure/manager.rs

pub struct StructureTemplateManager {
    /// Loaded templates, keyed by ResourceLocation.
    cache: HashMap<ResourceLocation, Arc<StructureTemplate>>,
    /// World folder path (for custom structures in world/structures/).
    world_dir: PathBuf,
}

impl StructureTemplateManager {
    pub fn get_or_load(&mut self, id: &ResourceLocation) -> Option<Arc<StructureTemplate>> {
        if let Some(t) = self.cache.get(id) {
            return Some(t.clone());
        }
        // Try world/structures/ first, then data pack
        let path = self.world_dir.join("structures")
            .join(id.path()).with_extension("nbt");
        let nbt = load_nbt_from_file(&path).ok()?;
        let template = StructureTemplate::from_nbt(&nbt)?;
        let arc = Arc::new(template);
        self.cache.insert(id.clone(), arc.clone());
        Some(arc)
    }
}
```

### 36.4 — Jigsaw placement (BFS assembly)

Villages and many other structures are assembled via the Jigsaw system.

#### 36.4.1 — Jigsaw block

Every `.nbt` piece may contain jigsaw blocks that mark connection points:

```rust
pub struct JigsawBlock {
    pub pos: BlockPos,
    pub facing: Direction,
    pub name: ResourceLocation,        // this connection's identity
    pub target: ResourceLocation,      // what this connects to
    pub pool: ResourceLocation,        // which pool provides the next piece
    pub joint_type: JigsawJointType,   // Rollable (any rotation) | Aligned (exact rotation)
    pub final_state: String,           // block to replace this jigsaw block with
}
```

#### 36.4.2 — BFS assembly algorithm

```rust
// crates/oxidized-world/src/structure/jigsaw.rs

pub fn generate_jigsaw(
    start_pool: &ResourceLocation,
    max_depth: u32,
    start_pos: BlockPos,
    level: &mut ServerLevel,
    pools: &StructureTemplatePools,
    templates: &mut StructureTemplateManager,
    rng: &mut impl RngCore,
) {
    let mut queue: VecDeque<PieceContext> = VecDeque::new();
    let start_piece = pools.get(start_pool)
        .and_then(|p| p.random_template(rng));
    if let Some(piece) = start_piece {
        queue.push_back(PieceContext { piece, pos: start_pos, depth: 0 });
    }
    let mut placed: Vec<BoundingBox> = Vec::new();

    while let Some(ctx) = queue.pop_front() {
        if ctx.depth >= max_depth { continue; }
        let template = templates.get_or_load(&ctx.piece.template).unwrap();
        let bb = ctx.bounding_box();

        // Check collision with already placed pieces
        if placed.iter().any(|p| p.intersects(&bb)) { continue; }
        placed.push(bb);
        ctx.piece.place(template, ctx.pos, level, rng);

        // For each jigsaw block in this piece, enqueue child pieces
        for jigsaw in template.jigsaw_blocks() {
            let world_jigsaw_pos = ctx.transform(jigsaw.pos);
            let target_pool = pools.get(&jigsaw.pool);
            if let Some(target_piece) = target_pool.and_then(|p| p.random_template(rng)) {
                // Find the connector in the target piece that matches jigsaw.target
                // Align the target piece so its connector abuts this jigsaw block
                if let Some(child_pos) = align_piece(&target_piece, &jigsaw, world_jigsaw_pos) {
                    queue.push_back(PieceContext {
                        piece: target_piece,
                        pos: child_pos,
                        depth: ctx.depth + 1,
                    });
                }
            }
        }
    }
}
```

**Depth limit:** `max_depth` is typically 7 for villages. When the depth is
exhausted, jigsaw blocks use their `final_state` (usually a plain block).

### 36.5 — Dungeon (Monster Room)

Simple structure, no Jigsaw:

- [ ] Place a 5×5 to 7×7 (random) room of cobblestone/mossy cobblestone walls
- [ ] Flat stone floor, no ceiling; carved into existing terrain
- [ ] 1 mob spawner in the center (zombie 50%, skeleton 25%, spider 25%)
- [ ] 1–2 chests with loot table `chests/simple_dungeon`
- [ ] Require the center to be in solid terrain (at least 3 solid sides)

```rust
// crates/oxidized-world/src/structure/structures/monster_room.rs

pub fn try_place(
    chunk_gen: &mut ChunkGenerator,
    origin: BlockPos,
    rng: &mut impl RngCore,
) -> bool {
    let width = rng.gen_range(2..=4) * 2 + 1;  // 5, 7
    let depth = rng.gen_range(2..=4) * 2 + 1;
    // Count solid/air blocks to decide validity ...
    // Place walls, floor, spawner, chests ...
}
```

### 36.6 — Mineshaft

Recursive corridor generator:

```rust
pub enum MineshaftPiece {
    Corridor { origin: BlockPos, length: i32, direction: Direction, has_rails: bool },
    CrossIntersection { origin: BlockPos },
    StairConnector { origin: BlockPos, direction: Direction },
    Room { origin: BlockPos },
}
```

- [ ] Each corridor is 3×2 (W×H) and 5–9 blocks long
- [ ] Corridors have a 70% chance of oak fence posts with wooden planks overhead
- [ ] Rails placed along corridor floor (rail + powered rail every 8 blocks)
- [ ] Cobwebs scattered at 60% density when near ore
- [ ] Ore veins can appear in walls (random_chance 0.1 per block)
- [ ] Cross intersections branch in 2–4 directions
- [ ] Maximum ~80 pieces total per mineshaft to prevent runaway generation
- [ ] Each piece checks collision with existing pieces before placement

### 36.7 — Stronghold

Strongholds are placed by `ConcentricRingsStructurePlacement` and assembled from
a catalog of room pieces:

| Piece type | Description |
|---|---|
| `StartingStairs` | Entry spiral staircase |
| `Straight` | Straight corridor |
| `PrisonHall` | Corridor with iron bars |
| `LeftTurn` / `RightTurn` | Corner pieces |
| `Crossing` | 3-way or 4-way intersection |
| `StairsStraight` | Descending stairs |
| `PortalRoom` | The End portal room |
| `Library` | Large bookshelf room |
| `StorageRoom` | Small room with chest |
| `SmoothStoneRoom` | Empty filler room |
| `ChestCorridor` | Corridor with chest |
| `FiveCrossing` | 5-exit hub room |

Assembly rules:
- Start from `StartingStairs`; BFS outward
- Portal room is placed exactly once per stronghold
- Maximum total piece count: 128

### 36.8 — `/locate` command

```
/locate structure <structure_id>   → nearest structure chunk
/locate biome <biome_id>           → nearest biome center
/locate poi <poi_type>             → nearest point of interest
```

Implementation: iterate outward in concentric rings of regions, checking
`potential_structure_chunk` against placement conditions until a valid start is
found or a maximum radius (128 chunks) is exceeded.

```rust
// crates/oxidized-game/src/command/locate.rs

pub fn locate_structure(
    level: &ServerLevel,
    origin: ChunkPos,
    structure_set_id: &ResourceLocation,
) -> Option<BlockPos> {
    let placement = level.structure_placements().get(structure_set_id)?;
    for radius in 0..128_i32 {
        for (rx, rz) in spiral_regions(radius) {
            let (cx, cz) = placement.potential_structure_chunk(
                level.seed(), rx, rz);
            if placement.is_valid_for_chunk(level, cx, cz) {
                return Some(BlockPos::new(cx * 16, 64, cz * 16));
            }
        }
    }
    None
}
```

---

## Acceptance Criteria

- [ ] A freshly generated world contains villages in plains/savanna/desert
      biomes at the correct placement density
- [ ] Dungeon rooms generate underground with a spawner and 1–2 chests
- [ ] Mineshafts generate with rails, cobwebs, and corridors
- [ ] Strongholds generate in concentric rings, each with a working End portal
      room
- [ ] `/locate structure minecraft:village` finds a village and opens a map
      pointing to it
- [ ] Structure templates loaded from `.nbt` files place correctly with all
      rotations
- [ ] Jigsaw BFS stops at `max_depth` and replaces remaining jigsaw blocks with
      their `final_state`
- [ ] Placed structures have the correct loot in their chests (Phase 34
      integration)
