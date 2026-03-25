# Implementation Phases

## Overview

Oxidized is built in **38+ phases**, each producing a small but tangible reward
(something visible/testable). Phases are sequenced so every phase builds on a
working foundation.

---

## Dependency Graph

```
P01 Bootstrap
 └─P02 TCP Framing
    └─P03 Handshake/Status  ← "appears in server list"
       └─P04 Login/Auth     ← "real client connects"
          └─P06 Configuration ←────────────────┐
P01──P05 NBT ──────────────────────────────────┘
P01──P07 Core Types
        └─P08 Block/Item Registry
              └─P09 Chunk Structures
                    ├─P23 Flat Worldgen
                    │      └─P26 Noise Worldgen
                    │             └─P36 Structures
                    └─P10 Anvil Loading
                           ├─P05 (NBT)
                           └─P11 Server Level
                                  └─P12 Player Join (also needs P06)
                                         ├─P13 Chunk Sending   ← "player sees world"
                                         │      └─P14 Movement  ← "can walk"
                                         │             └─P15 Entity Framework
                                         │                    ├─P16 Physics
                                         │                    │      └─P22 Block Interaction
                                         │                    │      │      ├─P23b ECS Runtime ← "entities in bevy_ecs"
                                         │                    │      │      │      ├─P23c Dropped Items ← "blocks drop loot"
                                         │                    │      │      │      │      └─P34 Loot Tables
                                         │                    │      │      │      └─P24 Combat
                                         │                    │      │      │             ├─P25 Hostile Mobs
                                         │                    │      │      │             └─P27 Animals
                                         │                    │      │      ├─P28 Redstone
                                         │                    │      │      └─P30 Block Entities
                                         │                    └─P19 World Ticking
                                         │                           ├─P20 World Saving
                                         │                           └─P38 Performance
                                         ├─P17 Chat
                                         ├─P18 Commands
                                         │      ├─P31 Advancements
                                         │      ├─P32 Scoreboard
                                         │      ├─P33 RCON/Query
                                         │      └─P37 JSON-RPC
                                         └─P21 Inventory
                                                ├─P22 Block Interaction
                                                └─P29 Crafting
                                                       └─P35 Enchantments
```

---

## Phase Index

| # | Phase | Crate(s) | Status | Reward |
|---|-------|----------|--------|--------|
| [01](phase-01-bootstrap.md) | Project Bootstrap | `server` | ✅ Complete | Compiles and runs |
| [02](phase-02-tcp-framing.md) | TCP + VarInt Framing | `protocol` | ✅ Complete | Accepts connections |
| [03](phase-03-handshake-status.md) | Handshake + Status | `protocol` | ✅ Complete | Server list ping |
| [04](phase-04-login-auth.md) | Login + Encryption | `protocol` | ✅ Complete | Client authenticates |
| [05](phase-05-nbt.md) | NBT | `nbt` | ✅ Complete | Read/write any `.dat` file |
| [06](phase-06-configuration.md) | Configuration State | `protocol` | ✅ Complete | Client reaches PLAY |
| [07](phase-07-core-types.md) | Core Data Types | `world` | ✅ Complete | Foundation types |
| [08](phase-08-block-item-registry.md) | Block & Item Registry | `world` | ✅ Complete | Look up any block by name |
| [09](phase-09-chunk-structures.md) | Chunk Structures | `world` | ✅ Complete | In-memory chunk |
| [10](phase-10-anvil-loading.md) | Anvil World Loading | `world` | ✅ Complete | Load real world from disk |
| [11](phase-11-server-level.md) | Server Level | `game` | ✅ Complete | Query any loaded block |
| [12](phase-12-player-join.md) | Player Join | `game` | ✅ Complete | Player enters world |
| [13](phase-13-chunk-sending.md) | Chunk Sending | `game` | ✅ Complete | Player sees terrain |
| [14](phase-14-player-movement.md) | Player Movement | `game` | ✅ Complete | Player walks |
| [15](phase-15-entity-framework.md) | Entity Framework | `game` | ✅ Complete | Entities visible to all |
| [16](phase-16-physics.md) | Physics | `game` | ✅ Complete | Gravity + collision |
| [17](phase-17-chat.md) | Chat | `game` | ✅ Complete | Players can chat |
| [18](phase-18-commands.md) | Commands | `game` | ✅ Complete | Core commands work |
| [19](phase-19-world-ticking.md) | World Ticking | `game` | ✅ Complete | Day/night cycle |
| [20](phase-20-world-saving.md) | World Saving | `game` | ✅ Complete | World persists |
| [21](phase-21-inventory.md) | Inventory | `game` | ✅ Complete | Inventory visible |
| [22](phase-22-block-interaction.md) | Block Interaction | `game` | ✅ Complete | Break/place blocks |
| [23](phase-23-flat-worldgen.md) | Flat World Generation | `world` | ✅ Complete | New worlds generate |
| [23a](phase-23a-lighting.md) | Lighting Engine | `game`, `world`, `protocol`, `server` | ✅ Complete | Correct sky & block light |
| [23b](phase-23b-ecs-runtime.md) | ECS Runtime Integration | `game`, `server` | 📋 Planned | bevy_ecs World drives entity tick |
| [23c](phase-23c-dropped-items.md) | Dropped Items | `game`, `server`, `protocol` | 📋 Planned | Blocks/players drop items |
| [24](phase-24-combat.md) | Combat | `game` | 📋 Planned | Take damage, die, respawn |
| [25](phase-25-hostile-mobs.md) | Hostile Mobs | `game` | 📋 Planned | Zombies attack you |
| [26](phase-26-noise-worldgen.md) | Noise World Gen | `world` | 📋 Planned | Full terrain generation |
| [27](phase-27-animals.md) | Animals | `game` | 📋 Planned | Cows/sheep/pigs graze |
| [28](phase-28-redstone.md) | Redstone | `game` | 📋 Planned | Basic circuits work |
| [29](phase-29-crafting.md) | Crafting | `game` | 📋 Planned | Craft items |
| [30](phase-30-block-entities.md) | Block Entities | `game` | 📋 Planned | Chests/furnaces work |
| [31](phase-31-advancements.md) | Advancements | `game` | 📋 Planned | Advancements pop up |
| [32](phase-32-scoreboard.md) | Scoreboard/Teams/Bossbar | `game` | 📋 Planned | Tab list scores |
| [33](phase-33-rcon-query.md) | RCON + Query | `server` | 📋 Planned | Remote management |
| [34](phase-34-loot-tables.md) | Loot Tables | `game` | 📋 Planned | Correct mob drops |
| [35](phase-35-enchantments.md) | Enchantments + Effects | `game` | 📋 Planned | Enchanting works |
| [36](phase-36-structures.md) | Structures | `world`+`game` | 📋 Planned | Villages generate |
| [37](phase-37-jsonrpc.md) | JSON-RPC Management | `server` | 📋 Planned | WebSocket API (26.1) |
| [38](phase-38-performance.md) | Performance Hardening | all | 📋 Planned | 100-player scale |
| [R1](phase-r1-refactoring.md) | Architectural Refactoring | `server`, `protocol`, `game` | ✅ Complete | Modular & extensible |
| [R2](phase-r2-refactoring.md) | Packet Trait & Unified Codec | `protocol`, `server` | ✅ Complete | Generic send/receive |
| [R3](phase-r3-refactoring.md) | ADR Compliance & Code Quality | all | 🔄 In Progress | Audit-clean codebase |

---

## Milestone Checkpoints

| After Phase | You can... |
|---|---|
| P03 | See the server in Minecraft's multiplayer list |
| P04 | Connect with a real vanilla 26.1 client |
| P12 | Log in and see the world loading screen |
| P13 | See actual terrain rendered |
| P14 | Walk around |
| P17 | Chat with other players |
| P18 | Run `/tp`, `/gamemode`, `/stop` |
| P20 | Restart and keep all progress |
| P22 | Play basic survival (dig, place) |
| P23b | Entity tick loop powered by bevy_ecs |
| P23c | Dropped items from blocks/players, pickup with animation |
| P25 | Play with mob danger |
| P29 | Craft tools and weapons |
| P38 | Host a public server |
