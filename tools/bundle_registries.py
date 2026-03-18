#!/usr/bin/env python3
"""Bundle Minecraft registry JSON files into a single registries.json.

Reads per-entry JSON files from mc-server-ref/mc-extracted/data/minecraft/
and writes a combined JSON file for compile-time inclusion in oxidized-protocol.

Usage:
    python3 tools/bundle_registries.py
"""

import json
import sys
from pathlib import Path

# Project root is one level up from this script.
PROJECT_ROOT = Path(__file__).resolve().parent.parent
EXTRACTED_DIR = PROJECT_ROOT / "mc-server-ref" / "mc-extracted" / "data" / "minecraft"
OUTPUT_FILE = PROJECT_ROOT / "crates" / "oxidized-protocol" / "src" / "data" / "registries.json"

# The 28 synchronized registries in vanilla order (from RegistryDataLoader.java).
SYNCHRONIZED_REGISTRIES = [
    "minecraft:worldgen/biome",
    "minecraft:chat_type",
    "minecraft:trim_pattern",
    "minecraft:trim_material",
    "minecraft:wolf_variant",
    "minecraft:wolf_sound_variant",
    "minecraft:pig_variant",
    "minecraft:pig_sound_variant",
    "minecraft:frog_variant",
    "minecraft:cat_variant",
    "minecraft:cat_sound_variant",
    "minecraft:cow_sound_variant",
    "minecraft:cow_variant",
    "minecraft:chicken_sound_variant",
    "minecraft:chicken_variant",
    "minecraft:zombie_nautilus_variant",
    "minecraft:painting_variant",
    "minecraft:dimension_type",
    "minecraft:damage_type",
    "minecraft:banner_pattern",
    "minecraft:enchantment",
    "minecraft:jukebox_song",
    "minecraft:instrument",
    "minecraft:test_environment",
    "minecraft:test_instance",
    "minecraft:dialog",
    "minecraft:world_clock",
    "minecraft:timeline",
]


def registry_dir(registry_name: str) -> Path:
    """Convert a registry name like 'minecraft:worldgen/biome' to its data directory."""
    # Strip the 'minecraft:' prefix to get the relative path.
    rel = registry_name.removeprefix("minecraft:")
    return EXTRACTED_DIR / rel


def bundle_registry(registry_name: str) -> dict:
    """Load all JSON entries for a single registry."""
    data_dir = registry_dir(registry_name)
    if not data_dir.is_dir():
        print(f"  WARNING: directory not found: {data_dir}", file=sys.stderr)
        return {}

    entries = {}
    for json_file in sorted(data_dir.glob("*.json")):
        entry_name = f"minecraft:{json_file.stem}"
        with open(json_file, "r", encoding="utf-8") as f:
            entries[entry_name] = json.load(f)

    return entries


def main() -> None:
    if not EXTRACTED_DIR.is_dir():
        print(f"ERROR: extracted data not found at {EXTRACTED_DIR}", file=sys.stderr)
        sys.exit(1)

    registries = {}
    total_entries = 0

    for reg_name in SYNCHRONIZED_REGISTRIES:
        entries = bundle_registry(reg_name)
        registries[reg_name] = entries
        count = len(entries)
        total_entries += count
        print(f"  {reg_name}: {count} entries")

    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(registries, f, separators=(",", ":"), ensure_ascii=False)

    size_kb = OUTPUT_FILE.stat().st_size / 1024
    print(f"\nBundled {total_entries} entries across {len(registries)} registries")
    print(f"Output: {OUTPUT_FILE} ({size_kb:.1f} KB)")


if __name__ == "__main__":
    main()
