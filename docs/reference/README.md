# Reference — Index

This directory contains low-level technical reference sheets that support the
implementation phases. They are static lookup documents, not design documents.

---

## Files

| File | Description |
|------|-------------|
| [java-class-map.md](java-class-map.md) | Maps every major Java class in the vanilla server to its planned Rust type and module. Use this to find the authoritative Java implementation of any feature. |
| [data-formats.md](data-formats.md) | Binary format reference: VarInt/VarLong, NBT, region files, chunk wire format, BlockPos encoding, entity metadata, UUID, login encryption sequence. |
| [protocol-packets.md](protocol-packets.md) | Complete listing of all Minecraft 26.1 packets: handshaking, status, login, configuration, and play (127 clientbound + 58 serverbound). |

---

## Decompiled Java Source

The decompiled vanilla reference is at:

```
mc-server-ref/decompiled/
```

This directory is **gitignored**. To regenerate the full reference setup
(download, extract, decompile, and run data generators) in one step:

```bash
./tools/setup-ref.sh
```

The script is idempotent — it skips any step that has already been completed.
It requires **Java 21+**, **curl**, and **jq**.

<details>
<summary>Manual steps (if you prefer)</summary>

1. Download the Minecraft 26.1 server JAR from the
   [Mojang version manifest](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)
2. Extract the bundled server from `META-INF/versions/`
3. Decompile with [Vineflower](https://github.com/Vineflower/vineflower):
   ```bash
   java -jar vineflower.jar mc-server-ref/extracted/server.jar mc-server-ref/decompiled/
   ```
4. Run the data generator:
   ```bash
   cd mc-server-ref && java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --all --output generated
   ```
5. Extract data from the inner JAR:
   ```bash
   cd mc-server-ref/mc-extracted && jar xf ../extracted/server.jar data/
   ```
</details>

The root packages of interest:

| Package path | Contents |
|---|---|
| `net/minecraft/server/` | Server main, dedicated server, RCON, Query, JSON-RPC |
| `net/minecraft/network/` | Protocol codec, packet classes, connection state |
| `net/minecraft/world/level/` | World, chunks, biomes, worldgen |
| `net/minecraft/world/entity/` | Entity hierarchy, AI, attributes |
| `net/minecraft/world/item/` | Item classes, enchantments, potions |
| `net/minecraft/world/level/storage/loot/` | Loot tables, conditions, functions |
| `net/minecraft/world/level/levelgen/` | Noise worldgen, structures, placements |
| `net/minecraft/nbt/` | NBT tag classes |
| `net/minecraft/commands/` | Command dispatcher, brigadier integration |
| `net/minecraft/advancements/` | Advancement tree, trigger system |
| `net/minecraft/core/registries/` | All built-in registries |

---

## Quick Lookup Tips

**Finding a packet class:**
```bash
grep -r "class Clientbound.*Packet\|class Serverbound.*Packet" \
  mc-server-ref/decompiled/net/minecraft/network/ \
  --include="*.java" -l
```

**Finding a registry entry:**
```bash
grep -r "register(" \
  mc-server-ref/decompiled/net/minecraft/core/registries/ \
  --include="*.java" | grep "your_thing"
```

**Finding all loot conditions:**
```bash
ls mc-server-ref/decompiled/net/minecraft/world/level/storage/loot/predicates/
```

**Finding all enchantment effect types:**
```bash
ls mc-server-ref/decompiled/net/minecraft/world/item/enchantment/effects/
```
