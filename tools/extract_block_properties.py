#!/usr/bin/env python3
"""Extract block properties from decompiled vanilla Blocks.java.

Parses BlockBehaviour.Properties builder chains from Blocks.java, cross-references
BlockEntityType.java for block entity ownership, and identifies interactable blocks
from useWithoutItem() overrides.

Outputs crates/oxidized-world/src/data/block_properties.json.gz — committed to the
repo and consumed by build.rs at compile time.

Usage:
    python3 tools/extract_block_properties.py
"""

import gzip
import json
import re
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
BLOCKS_JAVA = (
    PROJECT_ROOT
    / "mc-server-ref"
    / "decompiled"
    / "net"
    / "minecraft"
    / "world"
    / "level"
    / "block"
    / "Blocks.java"
)
BLOCK_ENTITY_TYPE_JAVA = (
    PROJECT_ROOT
    / "mc-server-ref"
    / "decompiled"
    / "net"
    / "minecraft"
    / "world"
    / "level"
    / "block"
    / "entity"
    / "BlockEntityType.java"
)
BLOCK_DIR = (
    PROJECT_ROOT
    / "mc-server-ref"
    / "decompiled"
    / "net"
    / "minecraft"
    / "world"
    / "level"
    / "block"
)
BLOCK_IDS_JAVA = (
    PROJECT_ROOT
    / "mc-server-ref"
    / "decompiled"
    / "net"
    / "minecraft"
    / "references"
    / "BlockIds.java"
)
OUTPUT_FILE = (
    PROJECT_ROOT / "crates" / "oxidized-world" / "src" / "data" / "block_properties.json.gz"
)


# ── Default property values (from BlockBehaviour.Properties constructor) ──────

DEFAULTS = {
    "has_collision": True,
    "is_air": False,
    "is_liquid": False,
    "is_replaceable": False,
    "is_opaque": True,       # canOcclude
    "is_flammable": False,   # ignitedByLava
    "requires_tool": False,
    "ticks_randomly": False,
    "force_solid_on": False,
    "force_solid_off": False,
    "light_emission": 0,
    "push_reaction": 0,      # 0=NORMAL, 1=DESTROY, 2=BLOCK, 3=PUSH_ONLY
    "friction": 0.6,
    "speed_factor": 1.0,
    "jump_factor": 1.0,
    "hardness": 0.0,
    "explosion_resistance": 0.0,
    "map_color": 0,           # MapColor.NONE
    "light_opacity": 0,       # derived heuristic: opaque+solid→15, liquid→1, else→0
}

PUSH_REACTION_MAP = {
    "PushReaction.NORMAL": 0,
    "PushReaction.DESTROY": 1,
    "PushReaction.BLOCK": 2,
    "PushReaction.PUSH_ONLY": 3,
    "PushReaction.IGNORE": 4,
}

# MapColor name → numeric ID (from MapColor.java constructor args).
# field_57 = ICE (renamed by decompiler).
MAP_COLOR_IDS = {
    "NONE": 0, "GRASS": 1, "SAND": 2, "WOOL": 3, "FIRE": 4,
    "field_57": 5, "METAL": 6, "PLANT": 7, "SNOW": 8, "CLAY": 9,
    "DIRT": 10, "STONE": 11, "WATER": 12, "WOOD": 13, "QUARTZ": 14,
    "COLOR_ORANGE": 15, "COLOR_MAGENTA": 16, "COLOR_LIGHT_BLUE": 17,
    "COLOR_YELLOW": 18, "COLOR_LIGHT_GREEN": 19, "COLOR_PINK": 20,
    "COLOR_GRAY": 21, "COLOR_LIGHT_GRAY": 22, "COLOR_CYAN": 23,
    "COLOR_PURPLE": 24, "COLOR_BLUE": 25, "COLOR_BROWN": 26,
    "COLOR_GREEN": 27, "COLOR_RED": 28, "COLOR_BLACK": 29,
    "GOLD": 30, "DIAMOND": 31, "LAPIS": 32, "EMERALD": 33,
    "PODZOL": 34, "NETHER": 35,
    "TERRACOTTA_WHITE": 36, "TERRACOTTA_ORANGE": 37,
    "TERRACOTTA_MAGENTA": 38, "TERRACOTTA_LIGHT_BLUE": 39,
    "TERRACOTTA_YELLOW": 40, "TERRACOTTA_LIGHT_GREEN": 41,
    "TERRACOTTA_PINK": 42, "TERRACOTTA_GRAY": 43,
    "TERRACOTTA_LIGHT_GRAY": 44, "TERRACOTTA_CYAN": 45,
    "TERRACOTTA_PURPLE": 46, "TERRACOTTA_BLUE": 47,
    "TERRACOTTA_BROWN": 48, "TERRACOTTA_GREEN": 49,
    "TERRACOTTA_RED": 50, "TERRACOTTA_BLACK": 51,
    "CRIMSON_NYLIUM": 52, "CRIMSON_STEM": 53, "CRIMSON_HYPHAE": 54,
    "WARPED_NYLIUM": 55, "WARPED_STEM": 56, "WARPED_HYPHAE": 57,
    "WARPED_WART_BLOCK": 58, "DEEPSLATE": 59, "RAW_IRON": 60,
    "GLOW_LICHEN": 61,
}

# DyeColor name → MapColor ID (from DyeColor.java).
# field_559 = RED (renamed by decompiler).
DYE_COLOR_TO_MAP_COLOR = {
    "WHITE": 8, "ORANGE": 15, "MAGENTA": 16, "LIGHT_BLUE": 17,
    "YELLOW": 18, "LIME": 19, "PINK": 20, "GRAY": 21,
    "LIGHT_GRAY": 22, "CYAN": 23, "PURPLE": 24, "BLUE": 25,
    "BROWN": 26, "GREEN": 27, "field_559": 28, "BLACK": 29,
}

# WeatheringCopperBlocks.create() produces 8 variants from a base name
WEATHERING_PREFIXES = [
    "", "exposed_", "weathered_", "oxidized_",
    "waxed_", "waxed_exposed_", "waxed_weathered_", "waxed_oxidized_",
]


def load_block_ids() -> dict[str, str]:
    """Load BlockIds.java to map constant names to minecraft: names."""
    mapping: dict[str, str] = {}
    if not BLOCK_IDS_JAVA.is_file():
        print(f"  WARNING: BlockIds.java not found at {BLOCK_IDS_JAVA}", file=sys.stderr)
        return mapping
    source = BLOCK_IDS_JAVA.read_text(encoding="utf-8")
    for m in re.finditer(r'ResourceKey<Block>\s+(\w+)\s*=\s*createKey\("([^"]+)"\)', source):
        mapping[m.group(1)] = m.group(2)
    return mapping


def parse_blocks_java(source: str, block_ids: dict[str, str]) -> dict[str, dict]:
    """Parse Blocks.java and extract per-block property data.

    Returns dict mapping minecraft:name to properties dict.
    """
    helpers = extract_helper_methods(source)

    blocks: dict[str, dict] = {}
    field_to_name: dict[str, str] = {}

    # Find all block field declarations:
    # - public static final Block FIELD = register(...);
    # - public static final WeatheringCopperBlocks FIELD = WeatheringCopperBlocks.create(...);
    field_pattern = re.compile(
        r'public\s+static\s+final\s+(?:Block|WeatheringCopperBlocks)\s+(\w+)\s*=\s*'
    )

    pos = 0
    while pos < len(source):
        m = field_pattern.search(source, pos)
        if not m:
            break

        field_name = m.group(1)
        start = m.end()
        stmt = extract_statement(source, start)
        pos = start + len(stmt)

        if "WeatheringCopperBlocks.create(" in stmt:
            # WeatheringCopperBlocks produces 8 variants
            base_name = extract_mc_name(stmt)
            if not base_name:
                continue
            props = extract_properties(stmt, field_to_name, blocks, helpers)
            for prefix in WEATHERING_PREFIXES:
                variant_name = f"minecraft:{prefix}{base_name}"
                blocks[variant_name] = dict(props)
            field_to_name[field_name] = f"minecraft:{base_name}"
        else:
            mc_name = extract_mc_name(stmt, block_ids)
            if not mc_name:
                continue
            full_name = f"minecraft:{mc_name}"
            field_to_name[field_name] = full_name
            props = extract_properties(stmt, field_to_name, blocks, helpers)
            blocks[full_name] = props

    return blocks, field_to_name


def extract_statement(source: str, start: int) -> str:
    """Extract a complete Java statement starting from `start`, handling nested parens."""
    depth = 0
    i = start
    while i < len(source):
        ch = source[i]
        if ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        elif ch == ';' and depth <= 0:
            return source[start:i]
        elif ch == '"':
            # Skip string literals
            i += 1
            while i < len(source) and source[i] != '"':
                if source[i] == '\\':
                    i += 1
                i += 1
        i += 1
    return source[start:]


def extract_mc_name(stmt: str, block_ids: dict[str, str] | None = None) -> str | None:
    """Extract the Minecraft block name from a register() call."""
    # Pattern 1: register("name", ...) or registerBed("name", ...) etc.
    m = re.search(r'register\w*\(\s*"([^"]+)"', stmt)
    if m:
        return m.group(1)
    # Pattern 2: register(BlockIds.FIELD, ...) — ResourceKey constant
    if block_ids:
        m = re.search(r'register\w*\(\s*BlockIds\.(\w+)', stmt)
        if m:
            return block_ids.get(m.group(1))
    # Pattern 3: WeatheringCopperBlocks.create("name", ...)
    m = re.search(r'WeatheringCopperBlocks\.create\(\s*"([^"]+)"', stmt)
    if m:
        return m.group(1)
    return None


def extract_helper_methods(source: str) -> dict[str, str]:
    """Extract helper method bodies from Blocks.java."""
    helpers = {}
    # Pattern: private static TYPE methodName(...) { ... }
    # We care about methods that return Properties or Block
    method_pattern = re.compile(
        r'(?:private|static)\s+(?:private\s+)?static\s+'
        r'(?:BlockBehaviour\.Properties|Block)\s+(\w+)\s*\('
    )
    pos = 0
    while pos < len(source):
        m = method_pattern.search(source, pos)
        if not m:
            break
        name = m.group(1)
        # Find the opening brace
        brace_pos = source.find('{', m.end())
        if brace_pos == -1:
            pos = m.end()
            continue
        # Find matching closing brace
        body = extract_braced_block(source, brace_pos)
        helpers[name] = body
        pos = brace_pos + len(body)
    return helpers


def extract_braced_block(source: str, start: int) -> str:
    """Extract a brace-delimited block starting at `start` (which should be '{')."""
    depth = 0
    i = start
    while i < len(source):
        ch = source[i]
        if ch == '{':
            depth += 1
        elif ch == '}':
            depth -= 1
            if depth == 0:
                return source[start:i + 1]
        elif ch == '"':
            i += 1
            while i < len(source) and source[i] != '"':
                if source[i] == '\\':
                    i += 1
                i += 1
        i += 1
    return source[start:]


def extract_properties(
    stmt: str,
    field_to_name: dict[str, str],
    existing_blocks: dict[str, dict],
    helpers: dict[str, str],
) -> dict:
    """Extract block properties from a Properties builder chain in a statement."""
    props = dict(DEFAULTS)

    # Determine the base: method_264() (fresh), ofFullCopy(BLOCK), or ofLegacyCopy(BLOCK)
    copy_match = re.search(r'of(?:Full|Legacy)Copy\(\s*(\w+)\s*\)', stmt)
    if copy_match:
        ref_field = copy_match.group(1)
        ref_name = field_to_name.get(ref_field)
        if ref_name and ref_name in existing_blocks:
            props = dict(existing_blocks[ref_name])

    # Check for helper method calls that return Properties
    for helper_name in ["logProperties", "netherStemProperties", "leavesProperties",
                        "shulkerBoxProperties", "pistonProperties", "buttonProperties",
                        "flowerPotProperties", "candleProperties"]:
        if helper_name + "(" in stmt:
            apply_helper_properties(props, helper_name, helpers)

    # wallVariant starts from defaults (method_264()) — already handled by initial DEFAULTS

    # Apply builder method calls from the statement
    apply_builder_methods(props, stmt)

    # Handle mapColor(BLOCK.defaultMapColor()) — resolve from already-parsed blocks
    m = re.search(r'\.mapColor\((\w+)\.defaultMapColor\(\)', stmt)
    if m:
        ref_field = m.group(1)
        ref_name = field_to_name.get(ref_field)
        if ref_name and ref_name in existing_blocks:
            props["map_color"] = existing_blocks[ref_name].get("map_color", 0)

    return props


def apply_helper_properties(props: dict, helper_name: str, helpers: dict[str, str]) -> None:
    """Apply properties set by a helper method."""
    body = helpers.get(helper_name, "")
    if not body:
        return

    # Parse the helper body for builder calls
    apply_builder_methods(props, body)


def apply_builder_methods(props: dict, text: str) -> None:
    """Parse and apply builder method calls from a Properties chain."""
    # noCollision() — hasCollision=false, canOcclude=false
    if ".noCollision()" in text or "noCollision()" in text:
        props["has_collision"] = False
        props["is_opaque"] = False

    # noOcclusion() — canOcclude=false
    if ".noOcclusion()" in text or "noOcclusion()" in text:
        props["is_opaque"] = False

    # replaceable()
    if ".replaceable()" in text or re.search(r'\breplaceable\(\)', text):
        props["is_replaceable"] = True

    # method_265() — marks as air
    if "method_265()" in text:
        props["is_air"] = True

    # liquid()
    if ".liquid()" in text:
        props["is_liquid"] = True

    # ignitedByLava()
    if "ignitedByLava()" in text:
        props["is_flammable"] = True

    # requiresCorrectToolForDrops()
    if "requiresCorrectToolForDrops()" in text:
        props["requires_tool"] = True

    # randomTicks()
    if "randomTicks()" in text:
        props["ticks_randomly"] = True

    # forceSolidOn()
    if "forceSolidOn()" in text:
        props["force_solid_on"] = True
        props["force_solid_off"] = False

    # forceSolidOff()
    if "forceSolidOff()" in text:
        props["force_solid_off"] = True
        props["force_solid_on"] = False

    # friction(X)
    m = re.search(r'\.friction\((\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["friction"] = float(m.group(1))

    # speedFactor(X)
    m = re.search(r'\.speedFactor\((\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["speed_factor"] = float(m.group(1))

    # jumpFactor(X)
    m = re.search(r'\.jumpFactor\((\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["jump_factor"] = float(m.group(1))

    # strength(X, Y) — destroyTime=X, explosionResistance=Y
    # strength(X) — both=X
    # instabreak() — both=0.0
    if "instabreak()" in text:
        props["hardness"] = 0.0
        props["explosion_resistance"] = 0.0

    m = re.search(r'\.strength\((-?\d+\.?\d*)[Ff]?,\s*(-?\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["hardness"] = float(m.group(1))
        props["explosion_resistance"] = float(m.group(2))
    else:
        m = re.search(r'\.strength\((-?\d+\.?\d*)[Ff]?\)', text)
        if m:
            val = float(m.group(1))
            props["hardness"] = val
            props["explosion_resistance"] = val

    # destroyTime(X) — only sets hardness, not resistance
    m = re.search(r'\.destroyTime\((-?\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["hardness"] = float(m.group(1))

    # explosionResistance(X) — only sets resistance
    m = re.search(r'\.explosionResistance\((-?\d+\.?\d*)[Ff]?\)', text)
    if m:
        props["explosion_resistance"] = float(m.group(1))

    # pushReaction(PushReaction.X)
    m = re.search(r'\.pushReaction\((PushReaction\.\w+)\)', text)
    if m:
        props["push_reaction"] = PUSH_REACTION_MAP.get(m.group(1), 0)

    # mapColor(MapColor.X) — direct constant
    m = re.search(r'\.mapColor\(MapColor\.(\w+)\)', text)
    if m:
        props["map_color"] = MAP_COLOR_IDS.get(m.group(1), 0)

    # mapColor(DyeColor.X) — dye color mapping
    m = re.search(r'\.mapColor\(DyeColor\.(\w+)\)', text)
    if m:
        props["map_color"] = DYE_COLOR_TO_MAP_COLOR.get(m.group(1), 0)

    # lightLevel — various patterns
    parse_light_level(props, text)


def parse_light_level(props: dict, text: str) -> None:
    """Parse lightLevel() calls and extract the emission value."""
    # Pattern: lightLevel(statex -> N) — constant
    m = re.search(r'lightLevel\(\w+\s*->\s*(\d+)\)', text)
    if m:
        props["light_emission"] = int(m.group(1))
        return

    # Pattern: lightLevel(litBlockEmission(N)) — max when lit
    m = re.search(r'lightLevel\(litBlockEmission\((\d+)\)\)', text)
    if m:
        props["light_emission"] = int(m.group(1))
        return

    # Pattern: lightLevel(GlowLichenBlock.emission(N))
    m = re.search(r'lightLevel\(GlowLichenBlock\.emission\((\d+)\)\)', text)
    if m:
        props["light_emission"] = int(m.group(1))
        return

    # Pattern: lightLevel(CandleBlock.LIGHT_EMISSION) — max 4 candles × 3 = 12
    if "CandleBlock.LIGHT_EMISSION" in text:
        props["light_emission"] = 12
        return

    # Pattern: lightLevel(LightBlock.LIGHT_EMISSION) — max level 15
    if "LightBlock.LIGHT_EMISSION" in text:
        props["light_emission"] = 15
        return


def extract_block_entity_blocks(source: str) -> set[str]:
    """Extract block names that have associated block entities from BlockEntityType.java."""
    blocks_with_entities: set[str] = set()

    # Find all register() calls in BlockEntityType.java
    # Pattern: register("type_name", Factory::new, Blocks.FIELD1, Blocks.FIELD2, ...)
    # We need to extract Blocks.FIELD references and map them to block names

    # First, collect all Blocks.FIELD references
    field_refs = re.findall(r'Blocks\.(\w+)', source)
    return set(field_refs)


def extract_interactable_blocks() -> set[str]:
    """Find blocks whose Java classes override useWithoutItem() returning SUCCESS.

    Uses class hierarchy to include subclasses of interactable base classes.
    """
    if not BLOCK_DIR.is_dir():
        print(f"  WARNING: Block source directory not found: {BLOCK_DIR}", file=sys.stderr)
        return set()

    # Build class hierarchy (child → parent)
    parents: dict[str, str] = {}
    for java_file in BLOCK_DIR.glob("*.java"):
        source = java_file.read_text(encoding="utf-8")
        m = re.search(r'class\s+(\w+)\s+extends\s+(\w+)', source)
        if m:
            parents[m.group(1)] = m.group(2)

    # Find classes that directly override useWithoutItem
    direct_interactable: set[str] = set()
    for java_file in BLOCK_DIR.glob("*.java"):
        source = java_file.read_text(encoding="utf-8")
        if re.search(r'(?:public|protected)\s+\w+\s+useWithoutItem\s*\(', source):
            direct_interactable.add(java_file.stem)

    # Cache: class → is interactable (walks parent chain)
    cache: dict[str, bool] = {}

    def is_class_interactable(cls: str) -> bool:
        if cls in cache:
            return cache[cls]
        if cls in direct_interactable:
            cache[cls] = True
            return True
        parent = parents.get(cls)
        if parent:
            result = is_class_interactable(parent)
            cache[cls] = result
            return result
        cache[cls] = False
        return False

    return direct_interactable, parents, is_class_interactable


def extract_factory_class(stmt: str) -> str | None:
    """Extract the factory class name from a register() call.

    Returns the class used to construct the block, e.g.:
    - `ChestBlock::new` → "ChestBlock"
    - `p -> new ChestBlock(p)` → "ChestBlock"
    - `register("stone", Properties...)` → None (uses default Block::new)
    """
    # Pattern 1: ClassName::new (method reference)
    m = re.search(r'(\w+)::new', stmt)
    if m:
        cls = m.group(1)
        if cls != "Block" and cls != "Blocks":
            return cls

    # Pattern 2: p -> new ClassName(...) (lambda factory)
    m = re.search(r'->\s*new\s+(\w+)\s*\(', stmt)
    if m:
        cls = m.group(1)
        if cls != "Block":
            return cls

    return None


def map_interactable_classes_to_blocks(
    blocks_source: str, field_to_name: dict[str, str],
    is_class_interactable_fn,
) -> set[str]:
    """Map interactable Java class names to minecraft: block names.

    A block is interactable if its factory class (or a parent) overrides
    useWithoutItem().
    """
    interactable_blocks: set[str] = set()

    field_pattern = re.compile(
        r'public\s+static\s+final\s+(?:Block|WeatheringCopperBlocks)\s+(\w+)\s*=\s*'
    )

    pos = 0
    while pos < len(blocks_source):
        m = field_pattern.search(blocks_source, pos)
        if not m:
            break

        field_name = m.group(1)
        start = m.end()
        stmt = extract_statement(blocks_source, start)
        pos = start + len(stmt)

        mc_name = field_to_name.get(field_name)
        if not mc_name:
            continue

        factory_cls = extract_factory_class(stmt)
        if factory_cls and is_class_interactable_fn(factory_cls):
            if "WeatheringCopperBlocks.create(" in stmt:
                base = mc_name.removeprefix("minecraft:")
                for prefix in WEATHERING_PREFIXES:
                    interactable_blocks.add(f"minecraft:{prefix}{base}")
            else:
                interactable_blocks.add(mc_name)

    return interactable_blocks


def build_field_to_block_name_map(blocks_source: str, block_ids: dict[str, str]) -> dict[str, str]:
    """Build mapping from Java field names to minecraft: block names."""
    mapping: dict[str, str] = {}
    # Pattern 1: register("name", ...)
    pattern1 = re.compile(
        r'public\s+static\s+final\s+(?:Block|WeatheringCopperBlocks)\s+(\w+)\s*=\s*'
        r'(?:register\w*|WeatheringCopperBlocks\.create)\(\s*"([^"]+)"'
    )
    for m in pattern1.finditer(blocks_source):
        field_name = m.group(1)
        mc_name = f"minecraft:{m.group(2)}"
        mapping[field_name] = mc_name

    # Pattern 2: register(BlockIds.FIELD, ...)
    pattern2 = re.compile(
        r'public\s+static\s+final\s+Block\s+(\w+)\s*=\s*register\w*\(\s*BlockIds\.(\w+)'
    )
    for m in pattern2.finditer(blocks_source):
        field_name = m.group(1)
        block_id_field = m.group(2)
        mc_id = block_ids.get(block_id_field)
        if mc_id:
            mapping[field_name] = f"minecraft:{mc_id}"

    return mapping


def map_block_entity_fields_to_names(
    be_field_refs: set[str], field_to_name: dict[str, str]
) -> set[str]:
    """Convert Blocks.FIELD references to minecraft: block names."""
    names: set[str] = set()
    for field in be_field_refs:
        mc_name = field_to_name.get(field)
        if mc_name:
            names.add(mc_name)
    return names


def extract_shape_occlusion_classes() -> tuple[set[str], dict[str, str]]:
    """Find block classes that override useShapeForLightOcclusion().

    Returns (direct_overrides, parents) where direct_overrides is the set of
    class names that directly override the method, and parents maps child→parent.
    """
    if not BLOCK_DIR.is_dir():
        print(f"  WARNING: Block source directory not found: {BLOCK_DIR}", file=sys.stderr)
        return set(), {}

    parents: dict[str, str] = {}
    direct: set[str] = set()
    for java_file in BLOCK_DIR.glob("**/*.java"):
        source = java_file.read_text(encoding="utf-8")
        m = re.search(r'class\s+(\w+)\s+extends\s+(\w+)', source)
        if m:
            parents[m.group(1)] = m.group(2)
        if re.search(r'(?:public|protected)\s+boolean\s+useShapeForLightOcclusion\s*\(', source):
            cls_m = re.search(r'class\s+(\w+)', source)
            if cls_m:
                direct.add(cls_m.group(1))

    # Remove BlockBehaviour itself — its default returns false
    direct.discard("BlockBehaviour")

    return direct, parents


def is_shape_occlusion_class(cls: str, direct: set[str], parents: dict[str, str],
                              cache: dict[str, bool] | None = None) -> bool:
    """Check if a class uses shape-based light occlusion (directly or inherited)."""
    if cache is None:
        cache = {}
    if cls in cache:
        return cache[cls]
    if cls in direct:
        cache[cls] = True
        return True
    parent = parents.get(cls)
    if parent:
        result = is_shape_occlusion_class(parent, direct, parents, cache)
        cache[cls] = result
        return result
    cache[cls] = False
    return False


def map_shape_occlusion_classes_to_blocks(
    blocks_source: str, field_to_name: dict[str, str],
    direct: set[str], parents: dict[str, str],
) -> set[str]:
    """Map shape-occlusion Java class names to minecraft: block names.

    Uses two strategies:
    1. Factory class detection (for blocks registered with explicit class refs)
    2. Helper method detection (registerLegacyStair → StairBlock, etc.)
    """
    result: set[str] = set()
    cache: dict[str, bool] = {}

    # Helper methods in Blocks.java that create shape-occluding block types
    helper_class_map = {
        "registerLegacyStair": "StairBlock",
        "registerStair": "StairBlock",
    }

    field_pattern = re.compile(
        r'public\s+static\s+final\s+(?:Block|WeatheringCopperBlocks)\s+(\w+)\s*='
    )

    pos = 0
    while pos < len(blocks_source):
        m = field_pattern.search(blocks_source, pos)
        if not m:
            break
        field_name = m.group(1)
        start = m.end()
        stmt = extract_statement(blocks_source, start)
        pos = start + len(stmt)

        mc_name = field_to_name.get(field_name)
        if not mc_name:
            continue

        # Strategy 1: direct factory class
        factory_cls = extract_factory_class(stmt)
        if factory_cls and is_shape_occlusion_class(factory_cls, direct, parents, cache):
            if "WeatheringCopperBlocks.create(" in stmt:
                base = mc_name.removeprefix("minecraft:")
                for prefix in WEATHERING_PREFIXES:
                    result.add(f"minecraft:{prefix}{base}")
            else:
                result.add(mc_name)
            continue

        # Strategy 2: known helper methods
        for helper, cls in helper_class_map.items():
            if helper + "(" in stmt and is_shape_occlusion_class(cls, direct, parents, cache):
                result.add(mc_name)
                break

    return result


def compute_is_solid(props: dict) -> bool:
    """Compute IS_SOLID flag from available properties.

    In vanilla, isSolid = forceSolidOn || (!forceSolidOff && block.getSolidStatus(state))
    where getSolidStatus checks if collision shape is a full block.

    Without collision shapes, we approximate: solid if has collision AND is opaque,
    unless force flags override.
    """
    if props["force_solid_on"]:
        return True
    if props["force_solid_off"] or not props["has_collision"]:
        return False
    # Heuristic: blocks with collision and occlusion are likely solid
    return props["is_opaque"]


def main() -> None:
    if not BLOCKS_JAVA.is_file():
        print(f"ERROR: Blocks.java not found at {BLOCKS_JAVA}", file=sys.stderr)
        print("Run: tools/setup-ref.sh", file=sys.stderr)
        sys.exit(1)

    # Load BlockIds constants
    block_ids = load_block_ids()
    print(f"Loaded {len(block_ids)} BlockIds constants")

    print("Parsing Blocks.java for block properties...")
    blocks_source = BLOCKS_JAVA.read_text(encoding="utf-8")
    blocks, field_to_name = parse_blocks_java(blocks_source, block_ids)
    print(f"  Extracted properties for {len(blocks)} blocks")

    # Build field → name map (comprehensive, for block entity cross-referencing)
    full_field_map = build_field_to_block_name_map(blocks_source, block_ids)
    # Merge with parse results
    full_field_map.update(field_to_name)
    print(f"  Built field→name map: {len(full_field_map)} entries")

    # Extract block entity associations
    print("Parsing BlockEntityType.java for block entities...")
    if BLOCK_ENTITY_TYPE_JAVA.is_file():
        be_source = BLOCK_ENTITY_TYPE_JAVA.read_text(encoding="utf-8")
        be_field_refs = extract_block_entity_blocks(be_source)
        be_block_names = map_block_entity_fields_to_names(be_field_refs, full_field_map)
        print(f"  Found {len(be_block_names)} blocks with block entities")
    else:
        print("  WARNING: BlockEntityType.java not found", file=sys.stderr)
        be_block_names = set()

    # Extract interactable blocks using class hierarchy
    print("Scanning for interactable blocks (useWithoutItem overrides)...")
    _direct, _parents, is_class_interactable = extract_interactable_blocks()
    interactable_blocks = map_interactable_classes_to_blocks(
        blocks_source, full_field_map, is_class_interactable,
    )
    print(f"  Found {len(interactable_blocks)} interactable blocks")

    # Extract shape-based light occlusion blocks
    print("Scanning for shape-based light occlusion (useShapeForLightOcclusion overrides)...")
    so_direct, so_parents = extract_shape_occlusion_classes()
    shape_occlusion_blocks = map_shape_occlusion_classes_to_blocks(
        blocks_source, full_field_map, so_direct, so_parents,
    )
    print(f"  Found {len(shape_occlusion_blocks)} blocks with shape-based light occlusion")

    # Enrich blocks with block entity and interactable flags, compute is_solid
    for name, props in blocks.items():
        props["has_block_entity"] = name in be_block_names
        props["is_interactable"] = name in interactable_blocks
        props["use_shape_for_light_occlusion"] = name in shape_occlusion_blocks
        props["is_solid"] = compute_is_solid(props)
        # Derive light_opacity heuristic:
        #   Full opaque solid blocks → 15 (blocks all light)
        #   Liquids → 1 (attenuates light slightly)
        #   Everything else → 0 (transparent or partial)
        if props["is_air"]:
            props["light_opacity"] = 0
        elif props["is_liquid"]:
            props["light_opacity"] = 1
        elif props["is_opaque"] and props["has_collision"] and props["is_solid"]:
            props["light_opacity"] = 15
        else:
            props["light_opacity"] = 0

    # Remove internal-only fields not needed by build.rs
    for props in blocks.values():
        props.pop("force_solid_on", None)
        props.pop("force_solid_off", None)

    # Validate
    validate_blocks(blocks)

    # Write output
    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    json_bytes = json.dumps(blocks, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    with gzip.open(OUTPUT_FILE, "wb", compresslevel=9) as f:
        f.write(json_bytes)

    size_kb = OUTPUT_FILE.stat().st_size / 1024
    print(f"\nOutput: {OUTPUT_FILE} ({size_kb:.1f} KB)")
    print(f"Blocks: {len(blocks)}")

    # Print some spot checks
    spot_checks = [
        ("minecraft:stone", "hardness=1.5, explosion_resistance=6.0, requires_tool=True, is_interactable=False"),
        ("minecraft:air", "is_air=True, has_collision=False"),
        ("minecraft:water", "is_liquid=True, is_replaceable=True"),
        ("minecraft:ice", "friction=0.98"),
        ("minecraft:soul_sand", "speed_factor=0.4"),
        ("minecraft:honey_block", "speed_factor=0.4, jump_factor=0.5"),
        ("minecraft:glowstone", "light_emission=15"),
        ("minecraft:obsidian", "hardness=50.0, explosion_resistance=1200.0, is_interactable=False"),
        ("minecraft:bedrock", "hardness=-1.0"),
        ("minecraft:chest", "has_block_entity=True, is_interactable=True"),
        ("minecraft:dirt", "hardness=0.5, is_solid=True"),
        ("minecraft:crafting_table", "is_interactable=True"),
        ("minecraft:furnace", "has_block_entity=True, is_interactable=True"),
        ("minecraft:copper_bars", "requires_tool=True"),
    ]
    print("\nSpot checks:")
    all_ok = True
    for name, expected in spot_checks:
        if name in blocks:
            b = blocks[name]
            ok = True
            for part in expected.split(", "):
                key, val_str = part.split("=")
                actual = b.get(key)
                if val_str == "True":
                    expected_val = True
                elif val_str == "False":
                    expected_val = False
                else:
                    expected_val = float(val_str)
                if actual != expected_val:
                    print(f"  FAIL {name}: {key} expected {expected_val}, got {actual}")
                    ok = False
                    all_ok = False
            if ok:
                print(f"  OK   {name}: {expected}")
        else:
            print(f"  MISS {name}: not found!")
            all_ok = False

    if not all_ok:
        print("\nSome spot checks failed!", file=sys.stderr)
        sys.exit(1)


def validate_blocks(blocks: dict[str, dict]) -> None:
    """Run basic validation on extracted block data."""
    errors = 0

    # Air blocks should be air
    for name in ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"]:
        if name in blocks and not blocks[name]["is_air"]:
            print(f"  ERROR: {name} should be is_air=True", file=sys.stderr)
            errors += 1

    # Water and lava should be liquid
    for name in ["minecraft:water", "minecraft:lava"]:
        if name in blocks and not blocks[name]["is_liquid"]:
            print(f"  ERROR: {name} should be is_liquid=True", file=sys.stderr)
            errors += 1

    # Stone should be solid with collision
    if "minecraft:stone" in blocks:
        s = blocks["minecraft:stone"]
        if not s["has_collision"]:
            print("  ERROR: stone should have collision", file=sys.stderr)
            errors += 1
        if not s["is_solid"]:
            print("  ERROR: stone should be solid", file=sys.stderr)
            errors += 1

    if errors:
        print(f"\n  {errors} validation error(s)!", file=sys.stderr)
    else:
        print("  Validation passed")


if __name__ == "__main__":
    main()
