# Entity Mapping: Vanilla → Oxidized

This document maps vanilla Minecraft entity class fields to their Oxidized
ECS component equivalents. It is maintained incrementally as entity types
are implemented.

> **Status:** Skeleton — populated during feature phases (P15/P24/P25/P27).

---

## Base Entity (`net.minecraft.world.entity.Entity`)

| Vanilla Field | Type | Oxidized Component | Status |
|---------------|------|--------------------|--------|
| `position` | `Vec3` | `Position(DVec3)` | ⚠️ Scaffolded |
| `deltaMovement` | `Vec3` | `Velocity(DVec3)` | ⚠️ Scaffolded |
| `yRot`, `xRot` | `float` | `Rotation { yaw, pitch }` | ⚠️ Scaffolded |
| `onGround` | `boolean` | `OnGround(bool)` | ⚠️ Scaffolded |
| `fallDistance` | `float` | `FallDistance(f32)` | ⚠️ Scaffolded |
| `DATA_SHARED_FLAGS` | `byte` | `EntityFlags(u8)` | ⚠️ Scaffolded |
| `noPhysics` | — | — | 🔲 Not yet |
| `noGravity` | `boolean` | `NoGravity` (marker) | ⚠️ Scaffolded |
| `silent` | `boolean` | `Silent` (marker) | ⚠️ Scaffolded |
| `tickCount` | `int` | `TickCount(u32)` | ⚠️ Scaffolded |
| `uuid` | `UUID` | — | 🔲 Not yet (stored separately) |
| `type` | `EntityType<?>` | — | 🔲 Not yet (stored separately) |
| `boundingBox` | `AABB` | — | 🔲 Not yet |
| `DATA_CUSTOM_NAME` | `Optional<Component>` | — | 🔲 Not yet |
| `DATA_CUSTOM_NAME_VISIBLE` | `boolean` | — | 🔲 Not yet |
| `DATA_POSE` | `Pose` | — | 🔲 Not yet |
| `DATA_TICKS_FROZEN` | `int` | — | 🔲 Not yet |
| `DATA_AIR_SUPPLY` | `int` | — | 🔲 Not yet |

## LivingEntity (`net.minecraft.world.entity.LivingEntity`)

| Vanilla Field | Type | Oxidized Component | Status |
|---------------|------|--------------------|--------|
| `health` | `float` | `Health { current, max }` | ⚠️ Scaffolded |
| `getArmorValue()` | `int` | `ArmorValue(f32)` | ⚠️ Scaffolded |
| `absorptionAmount` | `float` | `AbsorptionAmount(f32)` | ⚠️ Scaffolded |
| equipment slots | `ItemStack[]` | `Equipment { slot_count }` | ⚠️ Placeholder |
| `activeEffects` | `Map<…>` | — | 🔲 Not yet |
| `deathTime` | `int` | — | 🔲 Not yet |
| `hurtTime` | `int` | — | 🔲 Not yet |
| `lastDamageSource` | `DamageSource` | — | 🔲 Not yet |
| `attributes` | `AttributeMap` | — | 🔲 Not yet |

## Player (`net.minecraft.world.entity.player.Player`)

| Vanilla Field | Type | Oxidized Component | Status |
|---------------|------|--------------------|--------|
| (marker) | — | `PlayerMarker` | ⚠️ Scaffolded |
| `inventory.selected` | `int` | `SelectedSlot(u8)` | ⚠️ Scaffolded |
| experience level | `int` | `ExperienceData { level, progress, total }` | ⚠️ Scaffolded |
| `abilities` | `Abilities` | — | 🔲 Not yet |
| `foodData` | `FoodData` | — | 🔲 Not yet |
| `gameMode` | `GameType` | — | 🔲 Not yet |
| `inventory` | `Inventory` | — | 🔲 Not yet |

## Mob Markers

| Vanilla Class | Oxidized Marker | Status |
|---------------|-----------------|--------|
| `Zombie` | `ZombieMarker` | ⚠️ Scaffolded |
| `Skeleton` | `SkeletonMarker` | ⚠️ Scaffolded |
| `Creeper` | `CreeperMarker` | ⚠️ Scaffolded |
| `Spider` | `SpiderMarker` | ⚠️ Scaffolded |
| `Enderman` | `EndermanMarker` | ⚠️ Scaffolded |
| `Slime` | `SlimeMarker` | ⚠️ Scaffolded |
| `Phantom` | `PhantomMarker` | ⚠️ Scaffolded |
| `Drowned` | `DrownedMarker` | ⚠️ Scaffolded |
| `Witch` | `WitchMarker` | ⚠️ Scaffolded |
| `Villager` | `VillagerMarker` | ⚠️ Scaffolded |
| `Chicken` | `ChickenMarker` | ⚠️ Scaffolded |
| `Cow` | `CowMarker` | ⚠️ Scaffolded |
| `Pig` | `PigMarker` | ⚠️ Scaffolded |
| `Sheep` | `SheepMarker` | ⚠️ Scaffolded |
| `Horse` | `HorseMarker` | ⚠️ Scaffolded |
| `Wolf` | `WolfMarker` | ⚠️ Scaffolded |
| `Cat` | `CatMarker` | ⚠️ Scaffolded |
| `Rabbit` | `RabbitMarker` | ⚠️ Scaffolded |
| `IronGolem` | `IronGolemMarker` | ⚠️ Scaffolded |

## Spawn Bundles

| Bundle | Components | Status |
|--------|-----------|--------|
| `BaseEntityBundle` | Position, Velocity, Rotation, OnGround, FallDistance, EntityFlags, TickCount | ⚠️ Scaffolded |
| `LivingEntityBundle` | BaseEntityBundle + Health, ArmorValue, AbsorptionAmount, Equipment | ⚠️ Scaffolded |
| `ZombieBundle` | LivingEntityBundle + ZombieMarker | ⚠️ Scaffolded |
| `SkeletonBundle` | LivingEntityBundle + SkeletonMarker | ⚠️ Scaffolded |
| `CreeperBundle` | LivingEntityBundle + CreeperMarker | ⚠️ Scaffolded |
| `CowBundle` | LivingEntityBundle + CowMarker | ⚠️ Scaffolded |
| `PlayerBundle` | LivingEntityBundle + PlayerMarker + SelectedSlot + ExperienceData | ⚠️ Scaffolded |

## Tick Phases (ADR-018)

| Phase | Description | Systems |
|-------|-------------|---------|
| `PreTick` | Increment TickCount, process spawns/despawns | 🔲 Not yet |
| `Physics` | Gravity, velocity, collisions | 🔲 Not yet |
| `Ai` | GoalSelector, pathfinding | 🔲 Not yet |
| `EntityBehavior` | Type-specific logic | 🔲 Not yet |
| `StatusEffects` | Potion effects | 🔲 Not yet |
| `PostTick` | Bounding boxes, chunk tracking | 🔲 Not yet |
| `NetworkSync` | Dirty data serialisation | 🔲 Not yet |

---

**Legend:**
- ⚠️ Scaffolded — type exists with placeholder/default values
- 🔲 Not yet — to be implemented in feature phases
- ✅ Complete — fully implemented with vanilla parity
