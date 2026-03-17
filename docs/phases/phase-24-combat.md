# Phase 24 — LivingEntity + Combat

**Crate:** `oxidized-game`  
**Reward:** Players take damage, die, respawn correctly; HUD shows health.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-018: Entity System](../adr/adr-018-entity-system.md) — ECS with bevy_ecs for data-oriented entity management


## Goal

Implement the `LivingEntity` extension of `Entity` with health, armor, effects,
and the full `hurt()` pipeline: invulnerability check → armor reduction →
protection reduction → apply damage → hurt animation → death sequence. Implement
player respawning and the attribute modifier system. Keep the client health HUD
in sync via `ClientboundSetHealthPacket` after every change.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Damage pipeline | `net.minecraft.world.entity.LivingEntity#hurt` |
| Player death & respawn | `net.minecraft.server.level.ServerPlayer#die` |
| Respawn handler | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleClientCommand` |
| Armor reduction formula | `net.minecraft.world.entity.CombatRules#getDamageAfterAbsorb` |
| Health packet | `net.minecraft.network.protocol.game.ClientboundSetHealthPacket` |
| Damage event packet | `net.minecraft.network.protocol.game.ClientboundDamageEventPacket` |
| Entity event packet | `net.minecraft.network.protocol.game.ClientboundEntityEventPacket` |
| Player combat kill packet | `net.minecraft.network.protocol.game.ClientboundPlayerCombatKillPacket` |
| Respawn packet | `net.minecraft.network.protocol.game.ClientboundRespawnPacket` |
| Attribute map | `net.minecraft.world.entity.ai.attributes.AttributeMap` |
| Attribute instance | `net.minecraft.world.entity.ai.attributes.AttributeInstance` |

---

## Tasks

### 24.1 — DamageSource (`oxidized-game/src/combat/damage.rs`)

```rust
#[derive(Debug, Clone)]
pub struct DamageSource {
    pub damage_type: DamageType,
    /// Entity that is the root cause (e.g. the player who fired the arrow)
    pub cause_entity: Option<EntityRef>,
    /// Direct source entity (e.g. the arrow itself)
    pub direct_entity: Option<EntityRef>,
    /// World position (for environmental damage like void or explosion)
    pub source_pos: Option<(f64, f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    InFire,
    LightningBolt,
    OnFire,
    Lava,
    HotFloor,
    InWall,
    Cramming,
    Drown,
    Starve,
    Cactus,
    Fall,
    FlyIntoWall,
    OutOfWorld,
    Generic,
    Magic,
    Wither,
    DragonBreath,
    DriedOut,
    SweetBerryBush,
    Freeze,
    Stalagmite,
    PlayerAttack,
    MobAttack,
    Arrow,
    Trident,
    Fireworks,
    Fireball,
    Explosion,
    Indirect,
    Sonic,
    Sting,
    Thorns,
    CampfireDamage,
}

impl DamageType {
    /// Registry id (index into the damage_type registry).
    pub fn registry_id(&self) -> i32 {
        // Assigned by data pack; these are stable vanilla values.
        match self {
            Self::InFire          =>  0,
            Self::LightningBolt   =>  1,
            Self::OnFire          =>  2,
            Self::Lava            =>  3,
            Self::HotFloor        =>  4,
            Self::InWall          =>  5,
            Self::Cramming        =>  6,
            Self::Drown           =>  7,
            Self::Starve          =>  8,
            Self::Cactus          =>  9,
            Self::Fall            => 10,
            Self::FlyIntoWall     => 11,
            Self::OutOfWorld      => 12,
            Self::Generic         => 13,
            Self::Magic           => 14,
            Self::Wither          => 15,
            Self::DragonBreath    => 16,
            Self::DriedOut        => 17,
            Self::SweetBerryBush  => 18,
            Self::Freeze          => 19,
            Self::Stalagmite      => 20,
            Self::PlayerAttack    => 21,
            Self::MobAttack       => 22,
            Self::Arrow           => 23,
            Self::Trident         => 24,
            Self::Fireworks       => 25,
            Self::Fireball        => 26,
            Self::Explosion       => 27,
            _ => 28,
        }
    }

    /// Whether this damage type bypasses armor.
    pub fn bypasses_armor(&self) -> bool {
        matches!(self,
            Self::InFire | Self::OnFire | Self::Drown | Self::Starve |
            Self::OutOfWorld | Self::Generic | Self::Magic | Self::Wither |
            Self::DragonBreath | Self::Freeze
        )
    }

    /// Whether this damage type is absolute (bypasses all reductions).
    pub fn bypasses_invulnerability(&self) -> bool {
        matches!(self, Self::OutOfWorld)
    }
}

impl DamageSource {
    pub fn player_attack(attacker: EntityRef) -> Self {
        Self {
            damage_type: DamageType::PlayerAttack,
            cause_entity: Some(attacker.clone()),
            direct_entity: Some(attacker),
            source_pos: None,
        }
    }

    pub fn fall() -> Self {
        Self { damage_type: DamageType::Fall, cause_entity: None, direct_entity: None, source_pos: None }
    }

    pub fn starve() -> Self {
        Self { damage_type: DamageType::Starve, cause_entity: None, direct_entity: None, source_pos: None }
    }

    pub fn void() -> Self {
        Self { damage_type: DamageType::OutOfWorld, cause_entity: None, direct_entity: None, source_pos: None }
    }
}
```

### 24.2 — LivingEntity struct (`oxidized-game/src/entity/living.rs`)

```rust
use std::collections::HashMap;
use uuid::Uuid;

pub struct LivingEntity {
    // --- Base Entity fields ---
    pub entity_id: i32,
    pub uuid: Uuid,
    pub position: Vec3,
    pub rotation: Rotation,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub no_physics: bool,
    pub removed: bool,

    // --- LivingEntity fields ---
    pub health: f32,
    pub max_health: f32,            // attribute, default 20.0
    pub absorption: f32,            // extra HP buffer, from golden apples
    pub armor: f32,                 // total armor points (0–20)
    pub armor_toughness: f32,       // reduces armor piercing (0–20)

    /// Ticks of invulnerability remaining after last hurt. Default 10.
    pub invulnerable_time: i32,
    pub invulnerable_time_max: i32, // usually 10 for players, 20 for monsters

    /// The damage amount of the last hit received (used for invulnerability check).
    pub last_hurt: f32,

    pub dead: bool,
    pub death_time: i32,            // counts 0→20 after death

    /// UUID of last player who dealt damage (for death messages and XP attribution).
    pub last_hurt_by_player: Option<Uuid>,

    /// Fall distance accumulated this tick; reset on landing.
    pub fall_distance: f32,

    pub active_effects: HashMap<MobEffectId, MobEffectInstance>,
    pub attributes: AttributeMap,
}

impl LivingEntity {
    pub fn new(entity_id: i32) -> Self {
        Self {
            entity_id,
            uuid: Uuid::new_v4(),
            position: Vec3::ZERO,
            rotation: Rotation { yaw: 0.0, pitch: 0.0 },
            velocity: Vec3::ZERO,
            on_ground: false,
            no_physics: false,
            removed: false,
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            armor: 0.0,
            armor_toughness: 0.0,
            invulnerable_time: 0,
            invulnerable_time_max: 10,
            last_hurt: 0.0,
            dead: false,
            death_time: 0,
            last_hurt_by_player: None,
            fall_distance: 0.0,
            active_effects: HashMap::new(),
            attributes: AttributeMap::default(),
        }
    }
}
```

### 24.3 — Armor reduction formula (`oxidized-game/src/combat/combat_rules.rs`)

```rust
/// Port of `CombatRules.getDamageAfterAbsorb`.
/// armor_value: total armor points (0–20)
/// toughness:   armor toughness (0–20)
/// damage:      incoming damage before armor
pub fn get_damage_after_absorb(damage: f32, armor_value: f32, toughness: f32) -> f32 {
    // Effective armor = clamp(armor - damage/(2 + toughness/4), armor/5, armor)
    let effective_armor = (armor_value - damage / (2.0 + toughness / 4.0))
        .max(armor_value / 5.0)
        .min(20.0);
    // Damage reduction = effective_armor / 25 (cap at 80%)
    let reduction = (effective_armor / 25.0).min(0.8);
    damage * (1.0 - reduction)
}

/// Port of `CombatRules.getDamageAfterMagicAbsorb` (Protection enchantment).
/// protection_level: sum of Protection enchantment levels on all armor pieces
pub fn get_damage_after_magic_absorb(damage: f32, protection_level: i32) -> f32 {
    let clamped = protection_level.clamp(0, 20);
    let reduction = clamped as f32 / 25.0; // each point = 4% reduction, max 80%
    damage * (1.0 - reduction)
}
```

### 24.4 — Attribute system (`oxidized-game/src/entity/attributes.rs`)

```rust
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeKey {
    MaxHealth,              // default 20.0, range 1.0–1024.0
    FollowRange,            // default 32.0
    KnockbackResistance,    // default 0.0, range 0.0–1.0
    MovementSpeed,          // default 0.7 (players: 0.1)
    FlyingSpeed,            // default 0.4
    AttackDamage,           // default 2.0
    AttackKnockback,        // default 0.0
    AttackSpeed,            // default 4.0
    ArmorValue,             // default 0.0, range 0–30
    ArmorToughness,         // default 0.0, range 0–20
    Luck,                   // default 0.0, range -1024–1024
    MaxAbsorption,          // default 0.0
    JumpStrength,           // horses
    SpawnReinforcements,    // zombies
    Scale,                  // 1.20.5+
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperation {
    /// result = base + sum(ADD_VALUE modifiers)
    AddValue = 0,
    /// result = result * (1 + sum(ADD_MULTIPLIED_BASE modifiers))
    AddMultipliedBase = 1,
    /// result = result * product((1 + modifier) for each ADD_MULTIPLIED_TOTAL)
    AddMultipliedTotal = 2,
}

#[derive(Debug, Clone)]
pub struct AttributeModifier {
    pub id: Uuid,
    pub name: String,
    pub value: f64,
    pub operation: AttributeOperation,
}

#[derive(Debug, Clone)]
pub struct AttributeInstance {
    pub key: AttributeKey,
    pub base_value: f64,
    pub modifiers: Vec<AttributeModifier>,
    cached_value: Option<f64>,
}

impl AttributeInstance {
    pub fn new(key: AttributeKey, base_value: f64) -> Self {
        Self { key, base_value, modifiers: Vec::new(), cached_value: None }
    }

    /// Calculate the final value applying all modifiers in order.
    pub fn value(&mut self) -> f64 {
        if let Some(cached) = self.cached_value { return cached; }

        let mut result = self.base_value;

        // Step 1: ADD_VALUE
        for m in &self.modifiers {
            if m.operation == AttributeOperation::AddValue {
                result += m.value;
            }
        }

        let base_with_add = result;

        // Step 2: ADD_MULTIPLIED_BASE
        let mut multiplier_sum = 0.0;
        for m in &self.modifiers {
            if m.operation == AttributeOperation::AddMultipliedBase {
                multiplier_sum += m.value;
            }
        }
        result = base_with_add * (1.0 + multiplier_sum);

        // Step 3: ADD_MULTIPLIED_TOTAL
        for m in &self.modifiers {
            if m.operation == AttributeOperation::AddMultipliedTotal {
                result *= 1.0 + m.value;
            }
        }

        self.cached_value = Some(result);
        result
    }

    pub fn add_modifier(&mut self, modifier: AttributeModifier) {
        self.modifiers.retain(|m| m.id != modifier.id); // deduplicate by UUID
        self.modifiers.push(modifier);
        self.cached_value = None;
    }

    pub fn remove_modifier(&mut self, id: Uuid) {
        self.modifiers.retain(|m| m.id != id);
        self.cached_value = None;
    }
}

#[derive(Debug, Clone)]
pub struct AttributeMap {
    pub instances: HashMap<AttributeKey, AttributeInstance>,
}

impl Default for AttributeMap {
    fn default() -> Self {
        let mut m = HashMap::new();
        m.insert(AttributeKey::MaxHealth,         AttributeInstance::new(AttributeKey::MaxHealth, 20.0));
        m.insert(AttributeKey::MovementSpeed,     AttributeInstance::new(AttributeKey::MovementSpeed, 0.1));
        m.insert(AttributeKey::FlyingSpeed,       AttributeInstance::new(AttributeKey::FlyingSpeed, 0.4));
        m.insert(AttributeKey::AttackDamage,      AttributeInstance::new(AttributeKey::AttackDamage, 2.0));
        m.insert(AttributeKey::AttackSpeed,       AttributeInstance::new(AttributeKey::AttackSpeed, 4.0));
        m.insert(AttributeKey::ArmorValue,        AttributeInstance::new(AttributeKey::ArmorValue, 0.0));
        m.insert(AttributeKey::ArmorToughness,    AttributeInstance::new(AttributeKey::ArmorToughness, 0.0));
        m.insert(AttributeKey::Luck,              AttributeInstance::new(AttributeKey::Luck, 0.0));
        m.insert(AttributeKey::MaxAbsorption,     AttributeInstance::new(AttributeKey::MaxAbsorption, 0.0));
        m.insert(AttributeKey::KnockbackResistance, AttributeInstance::new(AttributeKey::KnockbackResistance, 0.0));
        Self { instances: m }
    }
}
```

### 24.5 — LivingEntity::hurt pipeline (`oxidized-game/src/entity/living.rs`)

```rust
impl LivingEntity {
    /// Returns true if damage was actually applied.
    pub async fn hurt(
        &mut self,
        source: DamageSource,
        amount: f32,
        broadcaster: &mut dyn PacketBroadcaster,
    ) -> bool {
        // 1. Void damage bypasses invulnerability
        if !source.damage_type.bypasses_invulnerability() {
            if self.invulnerable_time > 0 && amount <= self.last_hurt {
                return false;
            }
        }

        // 2. Armor reduction
        let after_armor = if source.damage_type.bypasses_armor() {
            amount
        } else {
            get_damage_after_absorb(amount, self.armor, self.armor_toughness)
        };

        // 3. Protection enchantment reduction
        let protection_level = self.get_total_protection();
        let after_protection = get_damage_after_magic_absorb(after_armor, protection_level);

        // 4. Absorption
        let mut remaining = after_protection;
        if self.absorption > 0.0 {
            let absorbed = remaining.min(self.absorption);
            self.absorption -= absorbed;
            remaining -= absorbed;
        }

        // 5. Apply damage
        self.health = (self.health - remaining).max(0.0);
        self.invulnerable_time = self.invulnerable_time_max;
        self.last_hurt = amount;

        if let Some(attacker_uuid) = source.cause_entity.as_ref().and_then(|e| e.as_player_uuid()) {
            self.last_hurt_by_player = Some(attacker_uuid);
        }

        // 6. Damage event packet
        broadcaster.send_to_tracking(ClientboundDamageEventPacket {
            entity_id: self.entity_id,
            damage_type_id: source.damage_type.registry_id(),
            source_cause_entity_id: source.cause_entity.as_ref().map(|e| e.id()),
            source_direct_entity_id: source.direct_entity.as_ref().map(|e| e.id()),
            source_position: source.source_pos,
        }).await;

        // 7. Hurt animation
        broadcaster.send_to_tracking(ClientboundAnimatePacket {
            entity_id: self.entity_id,
            animation: AnimationId::TakeDamage, // 1
        }).await;

        // 8. Death?
        if self.health <= 0.0 {
            self.die(source, broadcaster).await;
        }

        true
    }

    pub async fn die(
        &mut self,
        source: DamageSource,
        broadcaster: &mut dyn PacketBroadcaster,
    ) {
        if self.dead { return; } // prevent double-death
        self.dead = true;
        self.health = 0.0;

        // Mob death animation
        broadcaster.send_to_tracking(ClientboundEntityEventPacket {
            entity_id: self.entity_id,
            event_id: 3, // mob death
        }).await;
    }
}
```

### 24.6 — ServerPlayer hurt + death + respawn (`oxidized-game/src/player/combat.rs`)

```rust
impl ServerPlayer {
    pub async fn hurt(
        &mut self,
        source: DamageSource,
        amount: f32,
    ) -> bool {
        let applied = self.living.hurt(source.clone(), amount, &mut self.conn).await;
        if applied {
            self.send_health_packet().await;
        }
        applied
    }

    pub async fn send_health_packet(&mut self) {
        self.conn.send_packet(ClientboundSetHealthPacket {
            health: self.living.health.max(0.0),
            food: self.food_level,
            saturation: self.food_saturation,
        }).await.ok();
    }

    pub async fn die(&mut self, source: DamageSource) {
        self.living.die(source.clone(), &mut self.conn).await;

        // Death message
        let message = build_death_message(self, &source);
        self.conn.send_packet(ClientboundPlayerCombatKillPacket {
            player_id: self.entity_id,
            message,
        }).await.ok();

        // Drop inventory (unless keepInventory)
        if !self.level().game_rules().get_bool(GameRuleKey::KeepInventory) {
            self.drop_all_items().await;
        }

        // Broadcast showDeathMessages
        if self.level().game_rules().get_bool(GameRuleKey::ShowDeathMessages) {
            self.server.broadcast_system(build_death_component(self, &source), false).await;
        }
    }

    pub async fn respawn(&mut self) {
        // Reset living entity state
        self.living.health = self.living.max_health;
        self.living.dead = false;
        self.living.death_time = 0;
        self.living.invulnerable_time = 0;
        self.living.fall_distance = 0.0;
        self.living.active_effects.clear();
        self.food_level = 20;
        self.food_saturation = 5.0;

        // Find spawn position
        let (spawn_x, spawn_y, spawn_z) = self.resolve_spawn_point().await;

        // Respawn packet
        self.conn.send_packet(ClientboundRespawnPacket {
            common_player_spawn_info: CommonPlayerSpawnInfo {
                dimension_type: self.dimension_type_id(),
                dimension: self.dimension.resource_key(),
                seed: self.server.level_seed(),
                game_type: self.game_mode,
                previous_game_type: self.previous_game_mode,
                debug: false,
                flat: self.server.is_flat_world(),
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            data_to_keep: RespawnDataFlags::KEEP_ALL_DATA,
        }).await.ok();

        self.position = Vec3::new(spawn_x as f64 + 0.5, spawn_y as f64, spawn_z as f64 + 0.5);
        self.teleport_to_spawn().await;
        self.send_health_packet().await;
        self.conn.send_inventory().await.ok();
    }

    /// Calculate fall damage, broadcast, and apply.
    pub async fn handle_fall(&mut self, fall_distance: f32) {
        if fall_distance <= 3.0 { return; }
        if !self.server.game_rules().get_bool(GameRuleKey::FallDamage) { return; }
        if self.game_mode == GameType::Creative || self.game_mode == GameType::Spectator { return; }

        let raw_damage = (fall_distance - 3.0).floor();
        // Feather Falling enchantment: reduces by 3 per level
        let boot_ff = self.living.get_enchantment_level(Enchantment::FeatherFalling) as f32;
        let damage = (raw_damage - boot_ff * 3.0).max(0.0);
        if damage > 0.0 {
            self.hurt(DamageSource::fall(), damage).await;
        }
    }
}
```

### 24.7 — Combat packets (`oxidized-protocol/src/packets/clientbound/game.rs`)

```rust
/// 0x55 – health/food HUD update
#[derive(Debug, Clone)]
pub struct ClientboundSetHealthPacket {
    pub health: f32,        // 0.0–20.0 (or higher with max health modifiers)
    pub food: i32,          // VarInt 0–20
    pub saturation: f32,    // 0.0–5.0
}

/// 0x1A – entity took damage event (for client VFX and death screen)
#[derive(Debug, Clone)]
pub struct ClientboundDamageEventPacket {
    pub entity_id: i32,                     // VarInt
    pub damage_type_id: i32,               // VarInt registry id
    pub source_cause_entity_id: Option<i32>, // VarInt, optional
    pub source_direct_entity_id: Option<i32>,// VarInt, optional
    pub source_position: Option<(f64, f64, f64)>,
}

/// 0x03 – entity animation (hurt, swing arm, etc.)
#[derive(Debug, Clone)]
pub struct ClientboundAnimatePacket {
    pub entity_id: i32,    // VarInt
    pub animation: u8,
    // animation values:
    //   0 = swing main hand
    //   1 = take damage
    //   2 = leave bed
    //   3 = swing offhand
    //   4 = critical effect
    //   5 = magic critical effect
}

#[derive(Debug, Clone, Copy)]
pub enum AnimationId {
    SwingMainHand  = 0,
    TakeDamage     = 1,
    LeaveBed       = 2,
    SwingOffHand   = 3,
    CritEffect     = 4,
    MagicCrit      = 5,
}

/// 0x1D – entity status event (mob death = 3, totem of undying = 35, etc.)
#[derive(Debug, Clone)]
pub struct ClientboundEntityEventPacket {
    pub entity_id: i32,
    pub event_id: i8,
    // player events: 9=death, 10=remove effect, 35=totem
    // living: 3=mob death, 20=spawn explosion, 21=guardian attack
}

/// 0x3D – player was killed (shows death screen)
#[derive(Debug, Clone)]
pub struct ClientboundPlayerCombatKillPacket {
    pub player_id: i32,    // VarInt
    pub message: Component,
}

/// 0x47 – respawn the player into a (possibly different) dimension
#[derive(Debug, Clone)]
pub struct ClientboundRespawnPacket {
    pub common_player_spawn_info: CommonPlayerSpawnInfo,
    pub data_to_keep: RespawnDataFlags,
}

bitflags::bitflags! {
    pub struct RespawnDataFlags: u8 {
        const KEEP_METADATA    = 0x01;
        const KEEP_ALL_DATA    = 0xFF;
    }
}

/// 0x2C – ServerboundClientCommandPacket: PERFORM_RESPAWN or OPEN_INVENTORY
#[derive(Debug, Clone)]
pub struct ServerboundClientCommandPacket {
    pub action: ClientCommandAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ClientCommandAction {
    PerformRespawn  = 0,
    OpenInventory   = 1,
}
```

---

## Data Structures

```rust
// oxidized-game/src/entity/effects.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobEffectId {
    Speed = 1,
    Slowness = 2,
    Haste = 3,
    MiningFatigue = 4,
    Strength = 5,
    InstantHealth = 6,
    InstantDamage = 7,
    JumpBoost = 8,
    Nausea = 9,
    Regeneration = 10,
    Resistance = 11,
    FireResistance = 12,
    WaterBreathing = 13,
    Invisibility = 14,
    Blindness = 15,
    NightVision = 16,
    Hunger = 17,
    Weakness = 18,
    Poison = 19,
    Wither = 20,
    HealthBoost = 21,
    Absorption = 22,
    Saturation = 23,
    Glowing = 24,
    Levitation = 25,
    Luck = 26,
    Unluck = 27,
    SlowFalling = 28,
    ConduitPower = 29,
    DolphinsGrace = 30,
    BadOmen = 31,
    HeroOfTheVillage = 32,
    Darkness = 33,
}

#[derive(Debug, Clone)]
pub struct MobEffectInstance {
    pub effect: MobEffectId,
    pub amplifier: u8,          // 0 = level I, 1 = level II, etc.
    pub duration: i32,          // remaining ticks; -1 = infinite
    pub ambient: bool,          // from beacon (reduced particles)
    pub visible_particles: bool,
    pub show_icon: bool,
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- Armor reduction ---

    #[test]
    fn no_armor_does_not_reduce_damage() {
        let result = get_damage_after_absorb(10.0, 0.0, 0.0);
        assert!((result - 10.0).abs() < 1e-4, "0 armor = no reduction, got {result}");
    }

    #[test]
    fn full_armor_20_reduces_to_20_percent() {
        // 20 armor, 0 toughness, moderate damage
        // effective_armor = clamp(20 - 10/(2+0), 20/5, 20) = clamp(15, 4, 20) = 15
        // reduction = 15/25 = 0.6
        // result = 10 * 0.4 = 4.0
        let result = get_damage_after_absorb(10.0, 20.0, 0.0);
        assert!((result - 4.0).abs() < 1e-3, "expected 4.0, got {result}");
    }

    #[test]
    fn armor_toughness_reduces_effective_armor_less() {
        // High toughness → denominator (2 + toughness/4) is larger → effective armor is higher
        let without_tough = get_damage_after_absorb(20.0, 10.0, 0.0);
        let with_tough    = get_damage_after_absorb(20.0, 10.0, 8.0);
        assert!(with_tough < without_tough,
            "higher toughness should give better armor reduction");
    }

    #[test]
    fn armor_reduction_capped_at_80_percent() {
        // Even with unrealistically high armor the max reduction is 80%
        let result = get_damage_after_absorb(100.0, 200.0, 20.0);
        // min(effective/25, 0.8) → 0.8 cap
        assert!(result >= 20.0 - 1e-3, "minimum 20% of damage must pass through");
        assert!(result <= 20.1, "should not exceed 20.0 for 100 damage at 80% cap");
    }

    #[test]
    fn void_damage_bypasses_armor() {
        let source = DamageSource::void();
        assert!(source.damage_type.bypasses_armor());
    }

    #[test]
    fn player_attack_does_not_bypass_armor() {
        let attacker = EntityRef::mock();
        let source = DamageSource::player_attack(attacker);
        assert!(!source.damage_type.bypasses_armor());
    }

    // --- Protection enchantment ---

    #[test]
    fn protection_20_reduces_by_80_percent() {
        let result = get_damage_after_magic_absorb(10.0, 20);
        assert!((result - 2.0).abs() < 1e-4, "Protection 20 = 80% → 2.0, got {result}");
    }

    #[test]
    fn protection_0_does_not_reduce_damage() {
        let result = get_damage_after_magic_absorb(10.0, 0);
        assert!((result - 10.0).abs() < 1e-4);
    }

    #[test]
    fn protection_above_20_is_clamped() {
        let result_20  = get_damage_after_magic_absorb(10.0, 20);
        let result_100 = get_damage_after_magic_absorb(10.0, 100);
        assert!((result_20 - result_100).abs() < 1e-4,
            "protection above 20 should have no additional effect");
    }

    // --- Attribute system ---

    #[test]
    fn attribute_base_value_returned_with_no_modifiers() {
        let mut attr = AttributeInstance::new(AttributeKey::MaxHealth, 20.0);
        assert!((attr.value() - 20.0).abs() < 1e-5);
    }

    #[test]
    fn attribute_add_value_modifier_sums_correctly() {
        let mut attr = AttributeInstance::new(AttributeKey::MaxHealth, 20.0);
        attr.add_modifier(AttributeModifier {
            id: Uuid::new_v4(),
            name: "Bonus HP".into(),
            value: 4.0,
            operation: AttributeOperation::AddValue,
        });
        assert!((attr.value() - 24.0).abs() < 1e-5);
    }

    #[test]
    fn attribute_multiplied_base_modifier() {
        let mut attr = AttributeInstance::new(AttributeKey::MovementSpeed, 10.0);
        // ADD_VALUE += 5 → 15; then ADD_MULTIPLIED_BASE *= (1 + 0.2) → 15 * 1.2 = 18
        attr.add_modifier(AttributeModifier {
            id: Uuid::new_v4(), name: "flat".into(),
            value: 5.0, operation: AttributeOperation::AddValue,
        });
        attr.add_modifier(AttributeModifier {
            id: Uuid::new_v4(), name: "speed boost".into(),
            value: 0.2, operation: AttributeOperation::AddMultipliedBase,
        });
        assert!((attr.value() - 18.0).abs() < 1e-4, "expected 18.0, got {}", attr.value());
    }

    #[test]
    fn attribute_modifier_deduplicates_by_uuid() {
        let mut attr = AttributeInstance::new(AttributeKey::AttackDamage, 2.0);
        let id = Uuid::new_v4();
        attr.add_modifier(AttributeModifier { id, name: "first".into(), value: 1.0, operation: AttributeOperation::AddValue });
        attr.add_modifier(AttributeModifier { id, name: "second".into(), value: 3.0, operation: AttributeOperation::AddValue });
        // Should use the latest value (3.0), not both
        assert!((attr.value() - 5.0).abs() < 1e-4, "expected 5.0 (2+3), got {}", attr.value());
    }

    #[test]
    fn attribute_remove_modifier_restores_base() {
        let mut attr = AttributeInstance::new(AttributeKey::MaxHealth, 20.0);
        let id = Uuid::new_v4();
        attr.add_modifier(AttributeModifier { id, name: "test".into(), value: 10.0, operation: AttributeOperation::AddValue });
        assert!((attr.value() - 30.0).abs() < 1e-5);
        attr.remove_modifier(id);
        assert!((attr.value() - 20.0).abs() < 1e-5);
    }

    // --- Invulnerability logic ---

    #[test]
    fn hurt_blocked_when_invulnerable_and_lower_damage() {
        // invulnerable_time > 0 and new damage <= last_hurt → return false
        let invuln_time = 5;
        let last_hurt = 5.0f32;
        let new_damage = 3.0f32;
        // Simulates the guard at the start of hurt()
        let should_block = invuln_time > 0 && new_damage <= last_hurt;
        assert!(should_block, "should be blocked by invulnerability");
    }

    #[test]
    fn hurt_allowed_when_new_damage_exceeds_last_hurt() {
        let invuln_time = 5;
        let last_hurt = 3.0f32;
        let new_damage = 8.0f32;
        let should_block = invuln_time > 0 && new_damage <= last_hurt;
        assert!(!should_block, "higher damage should pierce invulnerability");
    }

    // --- Fall damage ---

    #[test]
    fn fall_damage_zero_for_three_blocks_or_less() {
        let fall = 3.0f32;
        let damage = (fall - 3.0).max(0.0).floor();
        assert_eq!(damage, 0.0);
    }

    #[test]
    fn fall_damage_one_for_four_blocks() {
        let fall = 4.0f32;
        let damage = (fall - 3.0).max(0.0).floor();
        assert_eq!(damage, 1.0);
    }

    #[test]
    fn fall_damage_scales_linearly() {
        for blocks in 4..20 {
            let expected = (blocks - 3) as f32;
            let actual = (blocks as f32 - 3.0).max(0.0).floor();
            assert!((actual - expected).abs() < 1e-5, "blocks={blocks}");
        }
    }
}
```
