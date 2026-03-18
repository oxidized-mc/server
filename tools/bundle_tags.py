#!/usr/bin/env python3
"""Bundle Minecraft tag JSON files into a single tags.json.

Reads per-tag JSON files from mc-server-ref/mc-extracted/data/minecraft/tags/
and uses both the data-driven registries.json AND the vanilla reports registries.json
to resolve entry names to numeric IDs.

Output format:
{
  "minecraft:enchantment": {
    "minecraft:curse": [3, 7],
    "minecraft:exclusive_set/armor": [0, 1, 2, 3]
  },
  ...
}

Usage:
    python3 tools/bundle_tags.py
"""

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
TAGS_DIR = PROJECT_ROOT / "mc-server-ref" / "mc-extracted" / "data" / "minecraft" / "tags"
DATA_REGISTRIES_FILE = PROJECT_ROOT / "crates" / "oxidized-protocol" / "src" / "data" / "registries.json"
VANILLA_REGISTRIES_FILE = PROJECT_ROOT / "mc-server-ref" / "generated" / "reports" / "registries.json"
OUTPUT_FILE = PROJECT_ROOT / "crates" / "oxidized-protocol" / "src" / "data" / "tags.json"

# Tag registries that are synchronized during configuration.
SYNCHRONIZED_TAG_REGISTRIES = [
    "banner_pattern",
    "block",
    "damage_type",
    "dialog",
    "enchantment",
    "entity_type",
    "fluid",
    "game_event",
    "instrument",
    "item",
    "painting_variant",
    "point_of_interest_type",
    "potion",
    "timeline",
    "villager_trade",
    "worldgen",
]

# Worldgen sub-registries that have tags
WORLDGEN_TAG_SUBDIRS = [
    "worldgen/biome",
    "worldgen/flat_level_generator_preset",
    "worldgen/structure",
    "worldgen/world_preset",
]


def load_registries() -> dict:
    """Load registry ID maps from both data-driven and vanilla reports."""
    id_maps = {}

    # Data-driven registries (registries.json bundled from extracted data)
    with open(DATA_REGISTRIES_FILE, "r", encoding="utf-8") as f:
        data_registries = json.load(f)
    for reg_name, entries in data_registries.items():
        name_to_id = {}
        for idx, entry_name in enumerate(entries.keys()):
            name_to_id[entry_name] = idx
        id_maps[reg_name] = name_to_id

    # Built-in registries from vanilla reports (have protocol_id)
    with open(VANILLA_REGISTRIES_FILE, "r", encoding="utf-8") as f:
        vanilla_registries = json.load(f)
    for reg_name, reg_data in vanilla_registries.items():
        if reg_name in id_maps:
            continue  # Data-driven registry already loaded
        entries = reg_data.get("entries", {})
        name_to_id = {}
        for entry_name, entry_data in entries.items():
            name_to_id[entry_name] = entry_data["protocol_id"]
        if name_to_id:
            id_maps[reg_name] = name_to_id

    return id_maps


def load_tag_file(tag_file: Path) -> list:
    """Load a tag JSON file and return its values list."""
    with open(tag_file, "r", encoding="utf-8") as f:
        data = json.load(f)
    return data.get("values", [])


def resolve_tag_entries(
    registry_name: str,
    tag_dir: Path,
    tag_name: str,
    id_map: dict,
    resolved_cache: dict,
    resolving_stack: set,
) -> list:
    """Resolve a tag's entries to numeric IDs, handling #tag references."""
    cache_key = f"{registry_name}:{tag_name}"
    if cache_key in resolved_cache:
        return resolved_cache[cache_key]

    if cache_key in resolving_stack:
        print(f"  WARNING: circular tag reference: {cache_key}", file=sys.stderr)
        return []

    resolving_stack.add(cache_key)

    # Find the tag file
    tag_path = tag_dir / f"{tag_name}.json"
    if not tag_path.is_file():
        print(f"  WARNING: tag file not found: {tag_path}", file=sys.stderr)
        resolving_stack.discard(cache_key)
        return []

    values = load_tag_file(tag_path)
    result_ids = []

    for value in values:
        if isinstance(value, dict):
            # Optional entry: {"id": "minecraft:foo", "required": false}
            entry_name = value.get("id", "")
            required = value.get("required", True)
            if entry_name.startswith("#"):
                ref_tag = entry_name[1:].removeprefix("minecraft:")
                ref_ids = resolve_tag_entries(
                    registry_name, tag_dir, ref_tag, id_map, resolved_cache, resolving_stack
                )
                result_ids.extend(ref_ids)
            elif entry_name in id_map:
                result_ids.append(id_map[entry_name])
            elif not required:
                pass  # Optional, skip silently
            else:
                print(f"  WARNING: entry '{entry_name}' not in registry '{registry_name}'", file=sys.stderr)
        elif isinstance(value, str):
            if value.startswith("#"):
                # Tag reference
                ref_tag = value[1:].removeprefix("minecraft:")
                ref_ids = resolve_tag_entries(
                    registry_name, tag_dir, ref_tag, id_map, resolved_cache, resolving_stack
                )
                result_ids.extend(ref_ids)
            elif value in id_map:
                result_ids.append(id_map[value])
            else:
                print(f"  WARNING: entry '{value}' not in registry '{registry_name}'", file=sys.stderr)

    resolving_stack.discard(cache_key)
    resolved_cache[cache_key] = result_ids
    return result_ids


def bundle_tags_for_registry(registry_name: str, tag_dir_name: str, id_maps: dict) -> dict:
    """Bundle all tags for a single registry."""
    tag_dir = TAGS_DIR / tag_dir_name
    if not tag_dir.is_dir():
        return {}

    # Find the corresponding registry ID map
    full_reg_name = f"minecraft:{tag_dir_name}"
    id_map = id_maps.get(full_reg_name, {})

    if not id_map:
        print(f"  WARNING: no ID map for {full_reg_name}, skipping", file=sys.stderr)
        return {}

    resolved_cache = {}
    tags = {}

    # Collect all tag files (including subdirectories like exclusive_set/)
    for tag_file in sorted(tag_dir.rglob("*.json")):
        rel_path = tag_file.relative_to(tag_dir)
        # Convert path to tag name: "exclusive_set/armor.json" → "exclusive_set/armor"
        tag_name = str(rel_path.with_suffix(""))
        full_tag_name = f"minecraft:{tag_name}"

        entries = resolve_tag_entries(
            full_reg_name, tag_dir, tag_name, id_map, resolved_cache, set()
        )

        tags[full_tag_name] = entries

    return tags


def main() -> None:
    if not TAGS_DIR.is_dir():
        print(f"ERROR: tags directory not found at {TAGS_DIR}", file=sys.stderr)
        sys.exit(1)

    if not DATA_REGISTRIES_FILE.is_file():
        print(f"ERROR: data registries.json not found at {DATA_REGISTRIES_FILE}", file=sys.stderr)
        sys.exit(1)

    if not VANILLA_REGISTRIES_FILE.is_file():
        print(f"ERROR: vanilla registries.json not found at {VANILLA_REGISTRIES_FILE}", file=sys.stderr)
        print("Run: cd mc-server-ref && java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --all --output generated", file=sys.stderr)
        sys.exit(1)

    print("Loading registries for ID mapping...")
    id_maps = load_registries()
    print(f"  Loaded {len(id_maps)} registry ID maps")

    all_tags = {}
    total_tags = 0

    for tag_reg in SYNCHRONIZED_TAG_REGISTRIES:
        if tag_reg == "worldgen":
            # Worldgen has sub-registries with separate tag directories
            for sub_reg in WORLDGEN_TAG_SUBDIRS:
                tag_dir = TAGS_DIR / sub_reg
                if not tag_dir.is_dir():
                    continue
                full_name = f"minecraft:{sub_reg}"
                tags = bundle_tags_for_registry(full_name, sub_reg, id_maps)
                if tags:
                    all_tags[full_name] = tags
                    count = len(tags)
                    total_tags += count
                    entries_total = sum(len(v) for v in tags.values())
                    print(f"  {full_name}: {count} tags, {entries_total} total entries")
        else:
            full_name = f"minecraft:{tag_reg}"
            tags = bundle_tags_for_registry(full_name, tag_reg, id_maps)
            if tags:
                all_tags[full_name] = tags
                count = len(tags)
                total_tags += count
                entries_total = sum(len(v) for v in tags.values())
                print(f"  {full_name}: {count} tags, {entries_total} total entries")

    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(all_tags, f, separators=(",", ":"), ensure_ascii=False)

    size_kb = OUTPUT_FILE.stat().st_size / 1024
    print(f"\nBundled {total_tags} tags across {len(all_tags)} registries")
    print(f"Output: {OUTPUT_FILE} ({size_kb:.1f} KB)")


if __name__ == "__main__":
    main()
