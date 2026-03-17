# Java Class → Rust Module Mapping

This table maps every major class in the vanilla Java server to its planned
Rust equivalent in Oxidized. Use it to navigate between the Java reference
source and the Rust implementation.

Columns:
- **Java Class** — short class name
- **Java Package** — fully qualified package
- **Rust Type** — planned Rust struct/enum/trait name
- **Rust Crate/Module** — `crate::module::path`
- **Notes** — key differences or special handling

---

## Network — TCP / Framing / Protocol State

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `Connection` | `net.minecraft.network` | `Connection` | `oxidized-protocol::connection` | Wraps tokio TcpStream + state machine |
| `PacketDecoder` | `net.minecraft.network` | `PacketReader` | `oxidized-protocol::connection::reader` | VarInt-length framed reads |
| `PacketEncoder` | `net.minecraft.network` | `PacketWriter` | `oxidized-protocol::connection::writer` | VarInt-length framed writes + BufWriter |
| `Varint21FrameDecoder` | `net.minecraft.network` | `FrameDecoder` | `oxidized-protocol::frame` | Splits byte stream into frames |
| `PacketListener` | `net.minecraft.network` | `PacketHandler` (trait) | `oxidized-protocol::handler` | Dispatch trait per state |
| `ConnectionProtocol` | `net.minecraft.network` | `ProtocolState` | `oxidized-protocol::state` | enum: Handshake/Status/Login/Config/Play |
| `ProtocolInfo` | `net.minecraft.network` | — | — | Packet ID ↔ type registry (const arrays) |
| `CompressionDecoder` | `net.minecraft.network` | `CompressionLayer` | `oxidized-protocol::compress` | zlib via `flate2` |
| `CompressionEncoder` | `net.minecraft.network` | `CompressionLayer` | `oxidized-protocol::compress` | threshold from oxidized.toml |
| `CipherDecoder` | `net.minecraft.network` | `CipherLayer` | `oxidized-protocol::cipher` | AES/CFB8 via `aes` crate |
| `CipherEncoder` | `net.minecraft.network` | `CipherLayer` | `oxidized-protocol::cipher` | same key, different IV state |
| `FriendlyByteBuf` | `net.minecraft.network` | `PacketBuf` | `oxidized-protocol::buf` | Trait impls over `bytes::Buf/BufMut` |
| `ClientboundPacket` | `net.minecraft.network.protocol` | `ClientboundPacket` (enum) | `oxidized-protocol::packets::clientbound` | One variant per packet |
| `ServerboundPacket` | `net.minecraft.network.protocol` | `ServerboundPacket` (enum) | `oxidized-protocol::packets::serverbound` | One variant per packet |

---

## Network — Login State

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `ClientboundHelloPacket` | `net.minecraft.network.protocol.login` | `ClientboundHelloPacket` | `oxidized-protocol::packets::clientbound::login` | Server public key + verify token |
| `ServerboundHelloPacket` | `net.minecraft.network.protocol.login` | `ServerboundHelloPacket` | `oxidized-protocol::packets::serverbound::login` | Username + UUID |
| `ServerboundKeyPacket` | `net.minecraft.network.protocol.login` | `ServerboundKeyPacket` | `oxidized-protocol::packets::serverbound::login` | RSA-encrypted shared secret |
| `ClientboundLoginFinishedPacket` | `net.minecraft.network.protocol.login` | `ClientboundLoginFinishedPacket` | `oxidized-protocol::packets::clientbound::login` | UUID + name |
| `ClientboundGameProfilePacket` | `net.minecraft.network.protocol.login` | — | — | Replaced by LoginFinished in 1.20.2+ |
| `ServerLoginPacketListenerImpl` | `net.minecraft.server.network` | `LoginHandler` | `oxidized-game::login` | Mojang auth → session DB lookup |

---

## Network — Game State Packets

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `ClientboundLevelChunkWithLightPacket` | `...protocol.game` | `ClientboundLevelChunkWithLightPacket` | `oxidized-protocol::packets::clientbound::game` | Full chunk + light data |
| `ClientboundSectionBlocksUpdatePacket` | `...protocol.game` | `ClientboundSectionBlocksUpdatePacket` | `...::game` | Batch block changes in a section |
| `ClientboundBlockUpdatePacket` | `...protocol.game` | `ClientboundBlockUpdatePacket` | `...::game` | Single block change |
| `ClientboundAddEntityPacket` | `...protocol.game` | `ClientboundAddEntityPacket` | `...::game` | Spawn entity (universal in 1.19+) |
| `ClientboundRemoveEntitiesPacket` | `...protocol.game` | `ClientboundRemoveEntitiesPacket` | `...::game` | Despawn list |
| `ClientboundMoveEntityPacket` | `...protocol.game` | `ClientboundMoveEntityPacket` | `...::game` | Relative move (delta ≤ 8 blocks) |
| `ClientboundTeleportEntityPacket` | `...protocol.game` | `ClientboundTeleportEntityPacket` | `...::game` | Absolute teleport |
| `ClientboundSetEntityDataPacket` | `...protocol.game` | `ClientboundSetEntityDataPacket` | `...::game` | Entity metadata list |
| `ClientboundSetEquipmentPacket` | `...protocol.game` | `ClientboundSetEquipmentPacket` | `...::game` | Armor/held item for entity |
| `ClientboundPlayerInfoUpdatePacket` | `...protocol.game` | `ClientboundPlayerInfoUpdatePacket` | `...::game` | Tab-list add/remove/update |
| `ClientboundRespawnPacket` | `...protocol.game` | `ClientboundRespawnPacket` | `...::game` | Sent on dimension change and respawn |
| `ClientboundSetDefaultSpawnPositionPacket` | `...protocol.game` | `ClientboundSetDefaultSpawnPositionPacket` | `...::game` | World spawn point |
| `ClientboundGameEventPacket` | `...protocol.game` | `ClientboundGameEventPacket` | `...::game` | Rain start/stop, demo events, etc. |
| `ClientboundSetTimePacket` | `...protocol.game` | `ClientboundSetTimePacket` | `...::game` | World time + day time |
| `ClientboundKeepAlivePacket` | `...protocol.game` | `ClientboundKeepAlivePacket` | `...::game` | Server→client keepalive payload |
| `ServerboundKeepAlivePacket` | `...protocol.game` | `ServerboundKeepAlivePacket` | `...::game` | Client echo response |
| `ClientboundSystemChatPacket` | `...protocol.game` | `ClientboundSystemChatPacket` | `...::game` | Server/system message |
| `ClientboundPlayerChatPacket` | `...protocol.game` | `ClientboundPlayerChatPacket` | `...::game` | Signed player chat message |
| `ClientboundUpdateMobEffectPacket` | `...protocol.game` | `ClientboundUpdateMobEffectPacket` | `...::game` | Apply/refresh potion effect |
| `ClientboundRemoveMobEffectPacket` | `...protocol.game` | `ClientboundRemoveMobEffectPacket` | `...::game` | Remove potion effect |
| `ClientboundOpenScreenPacket` | `...protocol.game` | `ClientboundOpenScreenPacket` | `...::game` | Open container UI |
| `ClientboundContainerSetContentPacket` | `...protocol.game` | `ClientboundContainerSetContentPacket` | `...::game` | Full inventory sync |
| `ClientboundContainerSetSlotPacket` | `...protocol.game` | `ClientboundContainerSetSlotPacket` | `...::game` | Single slot update |
| `ClientboundSetHealthPacket` | `...protocol.game` | `ClientboundSetHealthPacket` | `...::game` | HP + food + saturation |
| `ClientboundUpdateAttributesPacket` | `...protocol.game` | `ClientboundUpdateAttributesPacket` | `...::game` | Attribute modifiers |
| `ClientboundAwardStatsPacket` | `...protocol.game` | `ClientboundAwardStatsPacket` | `...::game` | Statistics update |

---

## NBT

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `Tag` | `net.minecraft.nbt` | `NbtTag` (enum) | `oxidized-nbt::tag` | 13 variants matching tag IDs |
| `CompoundTag` | `net.minecraft.nbt` | `NbtCompound` | `oxidized-nbt::compound` | `HashMap<String, NbtTag>` + ordered iter |
| `ListTag` | `net.minecraft.nbt` | `NbtList` | `oxidized-nbt::list` | All elements same tag type |
| `ByteTag` | `net.minecraft.nbt` | `NbtTag::Byte(i8)` | `oxidized-nbt::tag` | |
| `ShortTag` | `net.minecraft.nbt` | `NbtTag::Short(i16)` | `oxidized-nbt::tag` | |
| `IntTag` | `net.minecraft.nbt` | `NbtTag::Int(i32)` | `oxidized-nbt::tag` | |
| `LongTag` | `net.minecraft.nbt` | `NbtTag::Long(i64)` | `oxidized-nbt::tag` | |
| `FloatTag` | `net.minecraft.nbt` | `NbtTag::Float(f32)` | `oxidized-nbt::tag` | |
| `DoubleTag` | `net.minecraft.nbt` | `NbtTag::Double(f64)` | `oxidized-nbt::tag` | |
| `StringTag` | `net.minecraft.nbt` | `NbtTag::String(String)` | `oxidized-nbt::tag` | MUTF-8 decoded to Rust String |
| `ByteArrayTag` | `net.minecraft.nbt` | `NbtTag::ByteArray(Vec<i8>)` | `oxidized-nbt::tag` | |
| `IntArrayTag` | `net.minecraft.nbt` | `NbtTag::IntArray(Vec<i32>)` | `oxidized-nbt::tag` | |
| `LongArrayTag` | `net.minecraft.nbt` | `NbtTag::LongArray(Vec<i64>)` | `oxidized-nbt::tag` | |
| `NbtIo` | `net.minecraft.nbt` | `nbt_io` | `oxidized-nbt::io` | Read/write compressed + uncompressed |
| `NbtAccounter` | `net.minecraft.nbt` | `NbtQuota` | `oxidized-nbt::io` | Byte budget to prevent DoS |

---

## World / Chunks

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `LevelChunk` | `net.minecraft.world.level.chunk` | `LevelChunk` | `oxidized-world::chunk` | 24 vertical sections |
| `LevelChunkSection` | `net.minecraft.world.level.chunk` | `ChunkSection` | `oxidized-world::chunk::section` | 16³ block + biome palette |
| `PalettedContainer` | `net.minecraft.world.level.chunk` | `PalettedContainer<T>` | `oxidized-world::chunk::palette` | Generic over block state / biome |
| `PalettedContainerRO` | `net.minecraft.world.level.chunk` | `PalettedContainerRO<T>` | `oxidized-world::chunk::palette` | Read-only view (for network) |
| `SimpleBitStorage` | `net.minecraft.util` | `PackedBits` | `oxidized-world::util::packed_bits` | Dense bit array for palette indices |
| `ZeroBitStorage` | `net.minecraft.util` | `ZeroBitStorage` | `oxidized-world::util::packed_bits` | Optimised: all values identical |
| `Heightmap` | `net.minecraft.world.level.levelgen` | `Heightmap` | `oxidized-world::chunk::heightmap` | `[u16; 256]` per heightmap type |
| `DataLayer` | `net.minecraft.world.level.chunk` | `LightArray` | `oxidized-world::chunk::light` | Nibble-per-block light array |
| `ChunkPos` | `net.minecraft.world.level` | `ChunkPos` | `oxidized-world::pos` | `(i32, i32)` chunk coords |
| `BlockPos` | `net.minecraft.core` | `BlockPos` | `oxidized-world::pos` | `(i32, i32, i32)` block coords |
| `SectionPos` | `net.minecraft.core` | `SectionPos` | `oxidized-world::pos` | `(i32, i32, i32)` section coords |
| `RegionFileStorage` | `net.minecraft.world.level.chunk.storage` | `RegionStorage` | `oxidized-world::anvil` | Reads `.mca` region files |
| `RegionFile` | `net.minecraft.world.level.chunk.storage` | `RegionFile` | `oxidized-world::anvil::region` | Single `.mca` file |
| `ChunkSerializer` | `net.minecraft.world.level.chunk.storage` | `ChunkSerializer` | `oxidized-world::anvil::serialize` | NBT ↔ LevelChunk conversion |
| `ServerLevel` | `net.minecraft.server.level` | `ServerLevel` | `oxidized-game::level` | Live world with all systems |
| `ServerChunkCache` | `net.minecraft.server.level` | `ChunkCache` | `oxidized-world::chunk::cache` | LRU + load/save pipeline |
| `Level` | `net.minecraft.world.level` | `Level` | `oxidized-game::level::base` | Shared read-only world interface |
| `WorldBorder` | `net.minecraft.world.level.border` | `WorldBorder` | `oxidized-game::level::border` | |
| `TickList` | `net.minecraft.world.ticks` | `TickScheduler` | `oxidized-game::level::ticks` | Scheduled block/fluid ticks |

---

## Entities

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `Entity` | `net.minecraft.world.entity` | `Entity` | `oxidized-game::entity` | Base struct; ECS-style components |
| `LivingEntity` | `net.minecraft.world.entity` | `LivingEntity` | `oxidized-game::entity::living` | HP, effects, attributes |
| `Mob` | `net.minecraft.world.entity` | `Mob` | `oxidized-game::entity::mob` | AI, pathfinding, loot table |
| `PathfinderMob` | `net.minecraft.world.entity.ai.goal` | `PathfinderMob` | `oxidized-game::entity::ai` | Uses `PathNavigation` |
| `Monster` | `net.minecraft.world.entity.monster` | `Monster` | `oxidized-game::entity::mob::monster` | Hostile mob base |
| `Animal` | `net.minecraft.world.entity.animal` | `Animal` | `oxidized-game::entity::mob::animal` | Passive mob base |
| `ServerPlayer` | `net.minecraft.server.level` | `ServerPlayer` | `oxidized-game::player` | Online player entity |
| `Zombie` | `net.minecraft.world.entity.monster` | `Zombie` | `oxidized-game::entity::mob::zombie` | |
| `Skeleton` | `net.minecraft.world.entity.monster` | `Skeleton` | `oxidized-game::entity::mob::skeleton` | |
| `Creeper` | `net.minecraft.world.entity.monster` | `Creeper` | `oxidized-game::entity::mob::creeper` | |
| `Cow` | `net.minecraft.world.entity.animal` | `Cow` | `oxidized-game::entity::mob::cow` | |
| `Sheep` | `net.minecraft.world.entity.animal` | `Sheep` | `oxidized-game::entity::mob::sheep` | |
| `Villager` | `net.minecraft.world.entity.npc` | `Villager` | `oxidized-game::entity::mob::villager` | |
| `ItemEntity` | `net.minecraft.world.entity` | `ItemEntity` | `oxidized-game::entity::item` | Dropped item on ground |
| `ExperienceOrb` | `net.minecraft.world.entity` | `ExperienceOrb` | `oxidized-game::entity::xp_orb` | |
| `EntityMetadata` | `net.minecraft.network.syncher` | `EntityMetadata` | `oxidized-game::entity::metadata` | Type-erased synced data map |
| `SynchedEntityData` | `net.minecraft.network.syncher` | `EntityDataMap` | `oxidized-game::entity::synced_data` | Server-side dirty tracking |
| `EntityDataAccessor` | `net.minecraft.network.syncher` | `EntityDataKey<T>` | `oxidized-game::entity::synced_data` | Typed key |
| `Attributes` | `net.minecraft.world.entity.ai.attributes` | `Attributes` | `oxidized-game::entity::attributes` | Max HP, speed, etc. |
| `AttributeInstance` | `net.minecraft.world.entity.ai.attributes` | `AttributeInstance` | `oxidized-game::entity::attributes` | Base + modifiers |
| `AttributeModifier` | `net.minecraft.world.entity.ai.attributes` | `AttributeModifier` | `oxidized-game::entity::attributes` | UUID + operation + value |
| `Goal` | `net.minecraft.world.entity.ai.goal` | `Goal` (trait) | `oxidized-game::entity::ai::goal` | AI goal interface |
| `GoalSelector` | `net.minecraft.world.entity.ai.goal` | `GoalSelector` | `oxidized-game::entity::ai` | Priority-ordered goal list |
| `PathNavigation` | `net.minecraft.world.entity.ai.navigation` | `Navigator` | `oxidized-game::entity::ai::navigation` | A* pathfinder |
| `Path` | `net.minecraft.world.entity.ai.navigation` | `Path` | `oxidized-game::entity::ai::navigation` | Waypoint list |
| `MobEffect` | `net.minecraft.world.effect` | `MobEffect` | `oxidized-game::effect` | Registry entry |
| `MobEffectInstance` | `net.minecraft.world.effect` | `MobEffectInstance` | `oxidized-game::effect` | Active effect with duration |

---

## Server Core

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `MinecraftServer` | `net.minecraft.server` | `MinecraftServer` | `oxidized-server::server` | Top-level; owns all systems |
| `DedicatedServer` | `net.minecraft.server.dedicated` | `DedicatedServer` | `oxidized-server::server` | Dedicated (headless) impl |
| `DedicatedServerProperties` | `net.minecraft.server.dedicated` | `ServerProperties` | `oxidized-server::config` | TOML config parsing |
| `Main` | `net.minecraft.server` | `main()` | `oxidized-server::main` | Entry point |
| `ServerInterface` | `net.minecraft.server` | `ServerInterface` (trait) | `oxidized-server::api` | Command execution + status |
| `ServerTickRateManager` | `net.minecraft.server` | `TickRateManager` | `oxidized-server::tick_rate` | TPS control + sprint mode |
| `WatchdogThread` | `net.minecraft.server` | `Watchdog` | `oxidized-server::watchdog` | Freeze detection |
| `CrashReport` | `net.minecraft` | `CrashReport` | `oxidized-server::crash` | Panic handler output |
| `CrashReportCategory` | `net.minecraft` | `CrashSection` | `oxidized-server::crash` | Labeled section |
| `SharedConstants` | `net.minecraft` | `constants` module | `oxidized-server::constants` | Protocol version, data version |
| `DetectedVersion` | `net.minecraft` | `Version` | `oxidized-server::version` | Build metadata |
| `Eula` | `net.minecraft.server` | `Eula` | `oxidized-server::eula` | eula.txt read/write |
| `ConsoleInput` | `net.minecraft.server` | `ConsoleInput` | `oxidized-server::console` | stdin reader → command queue |
| `RconThread` | `net.minecraft.server.rcon.thread` | `RconServer` | `oxidized-server::rcon` | TCP RCON server |
| `RconClient` | `net.minecraft.server.rcon.thread` | `RconClientHandler` | `oxidized-server::rcon::client` | Per-connection handler |
| `RconConsoleSource` | `net.minecraft.server.rcon` | `RconConsoleSource` | `oxidized-server::rcon` | Command source for RCON |
| `QueryThreadGs4` | `net.minecraft.server.rcon.thread` | `QueryServer` | `oxidized-server::query` | UDP GameSpy4 query |
| `ManagementServer` | `net.minecraft.server.jsonrpc` | `ManagementServer` | `oxidized-server::jsonrpc` | WebSocket JSON-RPC server |

---

## Data / Registry

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `Registry<T>` | `net.minecraft.core` | `Registry<T>` | `oxidized-world::registry` | ID ↔ object bimap |
| `BuiltInRegistries` | `net.minecraft.core.registries` | `BuiltInRegistries` | `oxidized-world::registry::builtin` | Static refs to all registries |
| `ResourceLocation` | `net.minecraft.resources` | `ResourceLocation` | `oxidized-world::resource` | `namespace:path` |
| `ResourceKey<T>` | `net.minecraft.resources` | `ResourceKey<T>` | `oxidized-world::resource` | Typed registry key |
| `Holder<T>` | `net.minecraft.core` | `Holder<T>` | `oxidized-world::registry` | `Direct` or `Reference` variant |
| `HolderSet<T>` | `net.minecraft.core` | `HolderSet<T>` | `oxidized-world::registry` | Named or inline holder set |
| `TagKey<T>` | `net.minecraft.tags` | `TagKey<T>` | `oxidized-world::tags` | `#namespace:path` tag ref |
| `Block` | `net.minecraft.world.level.block` | `Block` | `oxidized-world::block` | Block type (not state) |
| `BlockState` | `net.minecraft.world.level.block.state` | `BlockState` | `oxidized-world::block::state` | Block + property values |
| `BlockBehaviour` | `net.minecraft.world.level.block.state` | `BlockBehaviour` | `oxidized-world::block::behaviour` | Break speed, material, sounds |
| `Item` | `net.minecraft.world.item` | `Item` | `oxidized-world::item` | Item type |
| `ItemStack` | `net.minecraft.world.item` | `ItemStack` | `oxidized-world::item::stack` | Item + count + components |
| `DataComponents` | `net.minecraft.core.component` | `DataComponents` | `oxidized-world::component` | Typed component map |
| `Enchantment` | `net.minecraft.world.item.enchantment` | `Enchantment` | `oxidized-game::enchantment` | Data-driven in 1.21+ |
| `ItemEnchantments` | `net.minecraft.world.item.enchantment` | `ItemEnchantments` | `oxidized-game::enchantment` | Per-item enchant list |
| `Biome` | `net.minecraft.world.level.biome` | `Biome` | `oxidized-world::biome` | Climate + surface parameters |
| `Climate.Sampler` | `net.minecraft.world.level.biome` | `ClimateSampler` | `oxidized-world::worldgen::noise` | Multi-noise biome lookup |
| `GameRules` | `net.minecraft.world.level.gamerules` | `GameRules` | `oxidized-game::game_rules` | Key→value rule map |
| `DimensionType` | `net.minecraft.world.level.dimension` | `DimensionType` | `oxidized-world::dimension` | Overworld/Nether/End properties |

---

## Worldgen

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `ChunkGenerator` | `net.minecraft.world.level.chunk` | `ChunkGenerator` (trait) | `oxidized-world::worldgen` | Generate chunk from seed |
| `NoiseBasedChunkGenerator` | `net.minecraft.world.level.levelgen` | `NoiseChunkGenerator` | `oxidized-world::worldgen::noise` | Full noise worldgen |
| `FlatLevelSource` | `net.minecraft.world.level.levelgen.flat` | `FlatChunkGenerator` | `oxidized-world::worldgen::flat` | Superflat |
| `NoiseChunk` | `net.minecraft.world.level.levelgen` | `NoiseChunk` | `oxidized-world::worldgen::noise::chunk` | Per-chunk noise samples |
| `Blender` | `net.minecraft.world.level.levelgen` | `Blender` | `oxidized-world::worldgen::blend` | Old←→new chunk blending |
| `Aquifer` | `net.minecraft.world.level.levelgen` | `Aquifer` | `oxidized-world::worldgen::aquifer` | Water/lava placement |
| `OreVeinifier` | `net.minecraft.world.level.levelgen` | `OreVeinifier` | `oxidized-world::worldgen::ore` | Copper/iron vein noise |
| `NormalNoise` | `net.minecraft.world.level.levelgen.synth` | `NormalNoise` | `oxidized-world::worldgen::noise::normal` | Double-perlin noise |
| `PerlinNoise` | `net.minecraft.world.level.levelgen.synth` | `PerlinNoise` | `oxidized-world::worldgen::noise::perlin` | Fractional brownian motion |
| `ImprovedNoise` | `net.minecraft.world.level.levelgen.synth` | `ImprovedNoise` | `oxidized-world::worldgen::noise::improved` | Single-octave Perlin |
| `Structure` | `net.minecraft.world.level.levelgen.structure` | `Structure` (trait) | `oxidized-world::structure` | Structure type |
| `JigsawStructure` | `...structure.structures` | `JigsawStructure` | `oxidized-world::structure::jigsaw` | BFS piece assembly |
| `StructureTemplate` | `...templatesystem` | `StructureTemplate` | `oxidized-world::structure::template` | NBT block template |
| `StructureTemplateManager` | `...templatesystem` | `TemplateManager` | `oxidized-world::structure::template` | LRU cache |
| `StructurePlacement` | `...structure.placement` | `StructurePlacement` (trait) | `oxidized-world::structure::placement` | Where to try |
| `RandomSpreadStructurePlacement` | `...structure.placement` | `RandomSpreadPlacement` | `oxidized-world::structure::placement` | Grid-jitter placement |
| `MonsterRoomStructure` | `...structure.structures` | `MonsterRoom` | `oxidized-world::structure::structures` | Dungeon room |
| `MineshaftStructure` | `...structure.structures` | `Mineshaft` | `oxidized-world::structure::structures` | Recursive corridors |
| `StrongholdStructure` | `...structure.structures` | `Stronghold` | `oxidized-world::structure::structures` | Template rooms + end portal |
| `WorldgenRandom` | `net.minecraft.world.level.levelgen` | `WorldgenRandom` | `oxidized-world::worldgen::random` | Seeded Xoroshiro/Legacy |
| `XoroshiroRandomSource` | `net.minecraft.world.level.levelgen.synth` | `Xoroshiro128` | `oxidized-world::worldgen::random` | Fast RNG for worldgen |
| `LegacyRandomSource` | `net.minecraft.world.level.levelgen` | `LegacyRandom` | `oxidized-world::worldgen::random` | Java `Random` clone |

---

## Commands

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `Commands` | `net.minecraft.commands` | `CommandDispatcher` | `oxidized-game::commands` | Wraps `brigadier-rs` |
| `CommandSourceStack` | `net.minecraft.commands` | `CommandSource` | `oxidized-game::commands::source` | Execution context |
| `CommandSource` | `net.minecraft.commands` | `CommandOutput` (trait) | `oxidized-game::commands` | `sendMessage` etc. |
| `TeleportCommand` | `net.minecraft.server.commands` | `tp_command` | `oxidized-game::commands::tp` | |
| `GameModeCommand` | `net.minecraft.server.commands` | `gamemode_command` | `oxidized-game::commands::gamemode` | |
| `SayCommand` | `net.minecraft.server.commands` | `say_command` | `oxidized-game::commands::say` | |
| `GiveCommand` | `net.minecraft.server.commands` | `give_command` | `oxidized-game::commands::give` | |
| `KillCommand` | `net.minecraft.server.commands` | `kill_command` | `oxidized-game::commands::kill` | |
| `StopCommand` | `net.minecraft.server.commands` | `stop_command` | `oxidized-game::commands::stop` | |
| `LocateCommand` | `net.minecraft.server.commands` | `locate_command` | `oxidized-game::commands::locate` | Structure + biome |
| `TickCommand` | `net.minecraft.server.commands` | `tick_command` | `oxidized-game::commands::tick` | Sprint/step/rate |

---

## Loot Tables

| Java Class | Java Package | Rust Type | Rust Crate/Module | Notes |
|---|---|---|---|---|
| `LootTable` | `net.minecraft.world.level.storage.loot` | `LootTable` | `oxidized-game::loot::table` | |
| `LootPool` | `net.minecraft.world.level.storage.loot` | `LootPool` | `oxidized-game::loot::pool` | |
| `LootContext` | `net.minecraft.world.level.storage.loot` | `LootContext` | `oxidized-game::loot::context` | |
| `LootParams` | `net.minecraft.world.level.storage.loot` | `LootParams` | `oxidized-game::loot::context` | Builder pattern |
| `LootItemCondition` | `net.minecraft.world.level.storage.loot.predicates` | `LootCondition` (trait) | `oxidized-game::loot::condition` | |
| `LootItemFunction` | `net.minecraft.world.level.storage.loot.functions` | `LootFunction` (trait) | `oxidized-game::loot::function` | |
| `LootPoolEntryContainer` | `net.minecraft.world.level.storage.loot.entries` | `LootEntry` | `oxidized-game::loot::entry` | |
| `NumberProvider` | `net.minecraft.world.level.storage.loot.providers.number` | `NumberProvider` | `oxidized-game::loot::provider` | |
| `BuiltInLootTables` | `net.minecraft.world.level.storage.loot` | `BuiltInLootTables` | `oxidized-game::loot` | Registry of all built-in table IDs |
