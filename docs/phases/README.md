# Implementation Phases

## Overview

Oxidized is built in **38 phases**, each producing a small but tangible reward
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
                                         │                    │      │      ├─P28 Redstone
                                         │                    │      │      └─P30 Block Entities
                                         │                    │      └─P24 Combat
                                         │                    │             ├─P25 Hostile Mobs
                                         │                    │             │      └─P34 Loot Tables
                                         │                    │             └─P27 Animals
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

| # | Phase | Crate(s) | Reward |
|---|-------|----------|--------|
| [01](phase-01-bootstrap.md) | Project Bootstrap | `server` | Compiles and runs |
| [02](phase-02-tcp-framing.md) | TCP + VarInt Framing | `protocol` | Accepts connections |
| [03](phase-03-handshake-status.md) | Handshake + Status | `protocol` | Server list ping ✅ |
| [04](phase-04-login-auth.md) | Login + Encryption | `protocol` | Client authenticates ✅ |
| [05](phase-05-nbt.md) | NBT | `nbt` | Read/write any `.dat` file |
| [06](phase-06-configuration.md) | Configuration State | `protocol` | Client reaches PLAY |
| [07](phase-07-core-types.md) | Core Data Types | `world` | Foundation types |
| [08](phase-08-block-item-registry.md) | Block & Item Registry | `world` | Look up any block by name |
| [09](phase-09-chunk-structures.md) | Chunk Structures | `world` | In-memory chunk |
| [10](phase-10-anvil-loading.md) | Anvil World Loading | `world` | Load real world from disk |
| [11](phase-11-server-level.md) | Server Level | `game` | Query any loaded block |
| [12](phase-12-player-join.md) | Player Join | `game` | Player enters world ✅ |
| [13](phase-13-chunk-sending.md) | Chunk Sending | `game` | Player sees terrain ✅ |
| [14](phase-14-player-movement.md) | Player Movement | `game` | Player walks ✅ |
| [15](phase-15-entity-framework.md) | Entity Framework | `game` | Entities visible to all |
| [16](phase-16-physics.md) | Physics | `game` | Gravity + collision |
| [17](phase-17-chat.md) | Chat | `game` | Players can chat ✅ |
| [18](phase-18-commands.md) | Commands | `game` | Core commands work ✅ |
| [19](phase-19-world-ticking.md) | World Ticking | `game` | Day/night cycle ✅ |
| [20](phase-20-world-saving.md) | World Saving | `game` | World persists ✅ |
| [21](phase-21-inventory.md) | Inventory | `game` | Inventory visible |
| [22](phase-22-block-interaction.md) | Block Interaction | `game` | Break/place blocks ✅ |
| [23](phase-23-flat-worldgen.md) | Flat World Generation | `world` | New worlds generate |
| [24](phase-24-combat.md) | Combat | `game` | Take damage, die, respawn |
| [25](phase-25-hostile-mobs.md) | Hostile Mobs | `game` | Zombies attack you |
| [26](phase-26-noise-worldgen.md) | Noise World Gen | `world` | Full terrain generation |
| [27](phase-27-animals.md) | Animals | `game` | Cows/sheep/pigs graze |
| [28](phase-28-redstone.md) | Redstone | `game` | Basic circuits work |
| [29](phase-29-crafting.md) | Crafting | `game` | Craft items ✅ |
| [30](phase-30-block-entities.md) | Block Entities | `game` | Chests/furnaces work |
| [31](phase-31-advancements.md) | Advancements | `game` | Advancements pop up |
| [32](phase-32-scoreboard.md) | Scoreboard/Teams/Bossbar | `game` | Tab list scores |
| [33](phase-33-rcon-query.md) | RCON + Query | `server` | Remote management |
| [34](phase-34-loot-tables.md) | Loot Tables | `game` | Correct mob drops |
| [35](phase-35-enchantments.md) | Enchantments + Effects | `game` | Enchanting works |
| [36](phase-36-structures.md) | Structures | `world`+`game` | Villages generate |
| [37](phase-37-jsonrpc.md) | JSON-RPC Management | `server` | WebSocket API (26.1) |
| [38](phase-38-performance.md) | Performance Hardening | all | 100-player scale |

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
| P25 | Play with mob danger |
| P29 | Craft tools and weapons |
| P38 | Host a public server |
