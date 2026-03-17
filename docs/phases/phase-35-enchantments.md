# Phase 35 — Enchantments + Potion Effects

**Crate:** `oxidized-game`  
**Reward:** Enchanted tools deal correct extra damage and have correct special
behaviours; potions apply status effects that visually and mechanically affect
entities; the brewing stand works.

**Depends on:** Phase 21 (inventory), Phase 24 (combat), Phase 25 (hostile
mobs), Phase 29 (crafting), Phase 30 (block entities)

---

## Goal

Implement two closely related systems: enchantments (persistent bonuses on
items, evaluated at use time) and mob effects / potions (temporary status
effects applied to living entities). Both systems are data-driven from the
enchantment registry and effect registry introduced in 1.21+.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Enchantment holder | `Enchantment` | `net.minecraft.world.item.enchantment.Enchantment` |
| Item enchantments component | `ItemEnchantments` | `net.minecraft.world.item.enchantment.ItemEnchantments` |
| Enchantment effects | `EnchantmentEffectComponents` | `net.minecraft.world.item.enchantment.effects.EnchantmentEffectComponents` |
| Mob effect | `MobEffect` | `net.minecraft.world.effect.MobEffect` |
| Mob effect instance | `MobEffectInstance` | `net.minecraft.world.effect.MobEffectInstance` |
| Apply effect packet | `ClientboundUpdateMobEffectPacket` | `net.minecraft.network.protocol.game.ClientboundUpdateMobEffectPacket` |
| Remove effect packet | `ClientboundRemoveMobEffectPacket` | `net.minecraft.network.protocol.game.ClientboundRemoveMobEffectPacket` |
| Brewing stand entity | `BrewingStandBlockEntity` | `net.minecraft.world.level.block.entity.BrewingStandBlockEntity` |
| Potion item | `PotionItem` | `net.minecraft.world.item.PotionItem` |
| Potion contents | `PotionContents` | `net.minecraft.world.item.alchemy.PotionContents` |

---

## Tasks

### 35.1 — Enchantment registry data (`oxidized-game/src/enchantment/registry.rs`)

In 1.21+, enchantments are fully data-driven. Each enchantment is a JSON file
in `data/minecraft/enchantment/<name>.json`. The structure:

```json
{
  "description": { "translate": "enchantment.minecraft.sharpness" },
  "supported_items": "#minecraft:sword_enchantable",
  "primary_items": "#minecraft:sword_enchantable",
  "weight": 10,
  "max_level": 5,
  "min_cost": { "base": 1, "per_level_above_first": 11 },
  "max_cost": { "base": 21, "per_level_above_first": 11 },
  "anvil_cost": 1,
  "slots": ["mainhand"],
  "exclusiveSet": "#minecraft:exclusive_set/damage",
  "effects": { ... }
}
```

```rust
// crates/oxidized-game/src/enchantment/enchantment.rs

#[derive(Debug, Clone)]
pub struct Enchantment {
    pub description: Component,
    pub supported_items: HolderSet<Item>,
    pub primary_items: Option<HolderSet<Item>>,
    pub weight: u32,
    pub max_level: u32,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: EnchantmentSlots,
    pub exclusive_set: Option<HolderSet<Enchantment>>,
    pub effects: EnchantmentEffectComponents,
}

/// Bit-flags for which equipment slots this enchantment applies to.
bitflags::bitflags! {
    pub struct EnchantmentSlots: u32 {
        const ARMOR       = 0x001;
        const FEET        = 0x002;
        const LEGS        = 0x004;
        const CHEST       = 0x008;
        const HEAD        = 0x010;
        const MAINHAND    = 0x020;
        const OFFHAND     = 0x040;
        const BODY        = 0x080;
        // Compound
        const WEAPON      = Self::MAINHAND.bits() | Self::OFFHAND.bits();
        const TOOL        = Self::MAINHAND.bits();
        const ANY         = 0x1FF;
    }
}
```

### 35.2 — `ItemEnchantments` component

The `minecraft:enchantments` item component stores the list of enchantments:

```rust
// crates/oxidized-game/src/enchantment/item_enchantments.rs

#[derive(Debug, Clone, Default)]
pub struct ItemEnchantments {
    /// Ordered list; each enchantment appears at most once.
    pub entries: Vec<(Holder<Enchantment>, i32)>,
    pub show_in_tooltip: bool,
}

impl ItemEnchantments {
    pub fn get_level(&self, enchantment: &ResourceKey<Enchantment>) -> i32 {
        self.entries.iter()
            .find(|(h, _)| h.key() == enchantment)
            .map(|(_, lvl)| *lvl)
            .unwrap_or(0)
    }

    pub fn set(&mut self, enchantment: Holder<Enchantment>, level: i32) {
        if let Some(e) = self.entries.iter_mut().find(|(h, _)| h == &enchantment) {
            e.1 = level;
        } else {
            self.entries.push((enchantment, level));
        }
    }
}
```

### 35.3 — Enchantment effect application in combat

Apply enchantment effects when an entity attacks. Key enchantments and their
mechanical effects (all multiply by `(level - 1)` where noted):

#### Damage enchantments (applied to attack damage)

| Enchantment | Formula |
|---|---|
| **Sharpness** | `+0.5 × level` extra damage (all mobs) |
| **Smite** | `+2.5 × level` vs undead mobs |
| **Bane of Arthropods** | `+2.5 × level` vs arthropods + Slowness IV for 1–1.5s |
| **Power** (bow) | `+0.5 × (level + 1)` % damage |
| **Impaling** (trident) | `+2.5 × level` vs aquatic mobs |

```rust
// crates/oxidized-game/src/enchantment/effects.rs

pub fn calculate_enchanted_damage(
    base_damage: f32,
    weapon: &ItemStack,
    target: &LivingEntity,
) -> f32 {
    let enchants = weapon.get::<ItemEnchantments>();
    let mut bonus = 0.0_f32;
    let level = enchants.get_level(&enchantments::SHARPNESS);
    if level > 0 { bonus += level as f32 * 0.5; }
    // Smite
    let level = enchants.get_level(&enchantments::SMITE);
    if level > 0 && target.mob_type() == MobType::Undead {
        bonus += level as f32 * 2.5;
    }
    // Bane of Arthropods
    let level = enchants.get_level(&enchantments::BANE_OF_ARTHROPODS);
    if level > 0 && target.mob_type() == MobType::Arthropod {
        bonus += level as f32 * 2.5;
        target.add_effect(MobEffectInstance::new(
            MobEffect::Slowness, 20 + rng.gen_range(0..10), 3));
    }
    base_damage + bonus
}
```

#### Other weapon enchantments

| Enchantment | Behaviour |
|---|---|
| **Fire Aspect** | Set target on fire for `4 × level` seconds |
| **Knockback** | Add `0.5 × level` to knockback multiplier |
| **Looting** | +1 extra drop roll per level (passed to loot context, Phase 34) |
| **Sweeping Edge** | AoE sweep damage `= base × level / (level + 1)` |
| **Thorns** | 15% × level chance to deal 1–4 damage back to attacker |

#### Tool enchantments

| Enchantment | Behaviour |
|---|---|
| **Efficiency** | Mining speed `× (1 + level²)` |
| **Silk Touch** | Block drops itself instead of normal loot |
| **Fortune** | Multiplies ore drop count (see Phase 34 `apply_bonus`) |
| **Unbreaking** | Each use: `level/(level+1)` chance to not consume durability |
| **Mending** | XP orbs repair held/worn item: 2 durability per XP point |

#### Armor enchantments

| Enchantment | Behaviour |
|---|---|
| **Protection** | Each piece: reduce damage by `4 × level %`; all 4 pieces stack, max 80% |
| **Fire Protection** | Reduce fire damage + shorten burning time |
| **Blast Protection** | Reduce explosion damage + knockback |
| **Projectile Protection** | Reduce projectile damage |
| **Feather Falling** | Reduce fall damage by `12 × level %` |
| **Depth Strider** | Increase underwater movement speed |
| **Frost Walker** | Turn water to frosted ice underfoot |
| **Respiration** | +15s underwater breathing per level; random chance to avoid drowning |
| **Aqua Affinity** | Full mining speed underwater |

### 35.4 — Mob effects (`oxidized-game/src/effect/mod.rs`)

All 33 vanilla effects with their IDs and mechanics:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VanillaEffect {
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
    BadLuck = 27,
    SlowFalling = 28,
    ConduitPower = 29,
    DolphinsGrace = 30,
    BadOmen = 31,
    HeroOfTheVillage = 32,
    Darkness = 33,
}
```

Mechanics per effect:

| ID | Effect | Behaviour |
|----|--------|-----------|
| 1 | Speed | Walk speed `× (1 + 0.2 × amplifier)` |
| 2 | Slowness | Walk speed `× (1 - 0.15 × amplifier)`; min ~0.05 |
| 3 | Haste | Break speed `× (1 + 0.2 × amplifier)` |
| 4 | Mining Fatigue | Break speed × 0.3^(amplifier+1); max amplifier 3 → 0.027% |
| 5 | Strength | Melee damage `+ 3 × (amplifier + 1)` |
| 6 | Instant Health | Instantly heal `4 × 2^amplifier` HP; damages undead |
| 7 | Instant Damage | Instantly deal `6 × 2^amplifier` damage; heals undead |
| 8 | Jump Boost | Jump height `+ 0.1 × (amplifier + 1)` |
| 9 | Nausea | Screen wobble; no gameplay effect |
| 10 | Regeneration | Heal 1 HP every `max(1, 50 / (amplifier + 1))` ticks |
| 11 | Resistance | Damage reduction `20 × (amplifier + 1)%`; max 4 → 80% |
| 12 | Fire Resistance | Complete fire/lava immunity |
| 13 | Water Breathing | Oxygen timer frozen; no drowning |
| 14 | Invisibility | Model invisible; armor/held items still visible |
| 15 | Blindness | Fog at ~5 blocks; mob targeting range halved |
| 16 | Night Vision | Full brightness everywhere |
| 17 | Hunger | Exhaust `0.005 × (amplifier + 1)` per tick |
| 18 | Weakness | Melee damage `- 4 × (amplifier + 1)` |
| 19 | Poison | 1 damage every `max(1, 25 / (amplifier + 1))` ticks; min 1 HP |
| 20 | Wither | 1 damage every `max(1, 40 / (amplifier + 1))` ticks; can kill |
| 21 | Health Boost | Max HP `+ 4 × (amplifier + 1)` |
| 22 | Absorption | Absorption hearts `+ 4 × (amplifier + 1)` |
| 23 | Saturation | Instant food + saturation restore; `(amplifier + 1)` per tick |
| 24 | Glowing | Visible through walls (outline renderer); no combat change |
| 25 | Levitation | Float upward at `0.05 × (amplifier + 1)` blocks/tick |
| 26 | Luck | `+luck` to loot context |
| 27 | Bad Luck | `−luck` to loot context |
| 28 | Slow Falling | No fall damage; fall speed halved |
| 29 | Conduit Power | Haste + water breathing underwater |
| 30 | Dolphin's Grace | Fast swimming (3×); conflicts with water breathing |
| 31 | Bad Omen | 0–4 amplifier; entering village triggers raid of amplifier+1 waves |
| 32 | Hero of the Village | Villager trade discounts per amplifier level |
| 33 | Darkness | Periodic screen darkening pulse every 5s; reduced render dist |

### 35.5 — `MobEffectInstance`

```rust
// crates/oxidized-game/src/effect/instance.rs

#[derive(Debug, Clone)]
pub struct MobEffectInstance {
    pub effect: Holder<MobEffect>,
    pub duration: i32,        // ticks remaining; -1 = infinite (beacon)
    pub amplifier: u8,        // 0-indexed (amplifier=0 means "level I")
    pub ambient: bool,        // from beacon; subtler particles
    pub visible: bool,        // show particles
    pub show_icon: bool,      // show in HUD
    pub blend: Option<MobEffectInstance>, // hidden previous instance
}

impl MobEffectInstance {
    pub fn tick(&mut self) -> bool {
        if self.duration > 0 { self.duration -= 1; }
        if self.duration == 0 { return false; /* expired */ }
        self.effect.apply_effect_tick(self);
        true
    }
}
```

### 35.6 — Network: effect packets

**`ClientboundUpdateMobEffectPacket` (0x72):**

| Field | Type | Notes |
|-------|------|-------|
| `entity_id` | VarInt | Target entity |
| `effect_id` | VarInt | Effect registry ID |
| `amplifier` | VarInt | 0-indexed |
| `duration` | VarInt | Ticks; -1 = infinite |
| `flags` | u8 | bit 0=ambient, bit 1=visible, bit 2=show_icon |
| `has_factor_data` | bool | NBT factor codec (usually false) |

**`ClientboundRemoveMobEffectPacket` (0x40):**

| Field | Type |
|-------|------|
| `entity_id` | VarInt |
| `effect_id` | VarInt |

Send `UpdateMobEffect` when:
- An effect is applied (`add_effect`)
- An effect is refreshed (same effect, same or higher amplifier)
- An existing effect is upgraded (higher amplifier replaces lower)

Send `RemoveMobEffect` when:
- An effect expires
- An effect is manually cleared
- The entity dies (for persistence purposes — re-send on respawn if needed)

### 35.7 — Potion items

**Potion types and behaviour:**

| Item ID | Interaction | Effect delivery |
|---------|------------|-----------------|
| `minecraft:potion` | Right-click to drink | Apply effects to self |
| `minecraft:splash_potion` | Throw (ProjectileItemEntity) | Apply to entities within 4-block radius; 100% at center, falloff |
| `minecraft:lingering_potion` | Throw; creates cloud | Area cloud entity; apply to entities every 5 ticks for 30s |
| `minecraft:tipped_arrow` | Shot; hits entity | Apply effects at 1/8th duration |

Each potion item carries the `minecraft:potion_contents` component:

```rust
#[derive(Debug, Clone, Default)]
pub struct PotionContents {
    pub potion: Option<ResourceKey<Potion>>,          // base potion type
    pub custom_color: Option<u32>,                     // ARGB
    pub custom_effects: Vec<MobEffectInstance>,        // extra effects
}
```

### 35.8 — Brewing stand (`oxidized-game/src/block_entity/brewing_stand.rs`)

The brewing stand processes one ingredient against up to 3 potions per brew
cycle (400 ticks with 1 blaze powder as fuel).

Brewing recipe lookup:
1. Check the `PotionBrewing` registry (loaded from data pack `brewing_recipes/`)
2. Recipe format: `(input_potion_type, ingredient_item) → output_potion_type`
3. All 3 slots transform simultaneously if they hold the input type

Key vanilla recipes (representative sample):

| Ingredient | Input | Output |
|---|---|---|
| Nether Wart | Awkward | (base; no effect) |
| Sugar | Awkward | Swiftness |
| Blaze Powder | Awkward | Strength |
| Glistering Melon | Awkward | Healing |
| Spider Eye | Awkward | Poison |
| Fermented Spider Eye | Swiftness | Slowness |
| Redstone | any | Extended duration (×8/3) |
| Glowstone | any | Enhanced (level II) |
| Gunpowder | Potion | Splash Potion |
| Dragon's Breath | Splash | Lingering Potion |

```rust
pub fn tick_brewing_stand(be: &mut BrewingStandBlockEntity, level: &mut ServerLevel) {
    if be.fuel == 0 {
        if be.items[3].is(items::BLAZE_POWDER) {
            be.items[3].shrink(1);
            be.fuel = 20;
        } else { return; }
    }
    if be.brew_time > 0 {
        be.brew_time -= 1;
        if be.brew_time == 0 { do_brew(be, level); }
    } else if can_brew(be, level) {
        be.brew_time = 400;
    }
}
```

### 35.9 — Enchanting table (Phase 29 extension)

- [ ] Compute three offer levels: `(level_min, level_max)` for each slot
- [ ] Enchanting always costs XP levels + lapis lazuli (1/2/3 lapis per slot)
- [ ] `enchant_with_levels` loot function used internally to generate enchantments
- [ ] Bookshelf bookshelves within 1 block (diagonally) of the table raise max
      offer level (max 30 at 15 bookshelves)

---

## Acceptance Criteria

- [ ] A Sharpness V sword deals 3.0 extra damage per hit vs vanilla baseline
- [ ] Fire Aspect II sets target on fire for 8 seconds
- [ ] Fortune III on a diamond ore gives 2–4 diamonds (not always 1)
- [ ] Potion of Speed II increases player movement speed visibly
- [ ] Drinking a potion of Healing instantly restores hearts
- [ ] Splash potion of Poison hits nearby entities
- [ ] Brewing stand converts Awkward Potion + Sugar → Swiftness Potion in 400t
- [ ] `ClientboundUpdateMobEffectPacket` is sent to all viewers on effect apply
- [ ] `ClientboundRemoveMobEffectPacket` is sent when effect expires
- [ ] Protection armor correctly reduces incoming damage
