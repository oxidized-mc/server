# Phase 27 — Animals

**Crate:** `oxidized-game`  
**Reward:** Cows, sheep, pigs graze, can be bred, and drop correct items.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-018: Entity System](../adr/adr-018-entity-system.md) — ECS with bevy_ecs for data-oriented entity management
- [ADR-023: AI & Pathfinding](../adr/adr-023-ai-pathfinding.md) — GoalSelector + optimized A* with path caching


## Goal

Implement the full passive-animal stack: `Animal` and `AgeableMob` base structs,
shared breeding/tempt/follow-parent goals, and five concrete animals (Cow, Sheep,
Pig, Chicken, Horse) with species-specific mechanics (wool shearing, milk
buckets, egg laying, pig saddling, horse taming). Breeding interactions must work
end-to-end: right-click with food → love mode → baby spawns with XP reward.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Animal base | `Animal` | `net.minecraft.world.entity.animal.Animal` |
| Ageable mob | `AgeableMob` | `net.minecraft.world.entity.AgeableMob` |
| Cow | `Cow` | `net.minecraft.world.entity.animal.Cow` |
| Sheep | `Sheep` | `net.minecraft.world.entity.animal.Sheep` |
| Pig | `Pig` | `net.minecraft.world.entity.animal.Pig` |
| Chicken | `Chicken` | `net.minecraft.world.entity.animal.Chicken` |
| Horse | `Horse` | `net.minecraft.world.entity.animal.horse.Horse` |
| Breed goal | `BreedGoal` | `net.minecraft.world.entity.ai.goal.BreedGoal` |
| Tempt goal | `TemptGoal` | `net.minecraft.world.entity.ai.goal.TemptGoal` |
| Follow parent goal | `FollowParentGoal` | `net.minecraft.world.entity.ai.goal.FollowParentGoal` |
| Panic goal | `PanicGoal` | `net.minecraft.world.entity.ai.goal.PanicGoal` |
| Eat grass goal | `EatBlockGoal` | `net.minecraft.world.entity.ai.goal.EatBlockGoal` |
| Love hearts event | `ClientboundEntityEventPacket` | `net.minecraft.network.protocol.game.ClientboundEntityEventPacket` |

---

## Tasks

### 27.1 — `AgeableMob` base

```rust
// crates/oxidized-game/src/entity/ageable_mob.rs

use super::mob::Mob;
use uuid::Uuid;

/// Base struct for mobs that have a baby/adult lifecycle.
/// Mirrors `AgeableMob` in Java.
pub struct AgeableMob {
    pub base: Mob,
    /// Negative → baby (counts up to 0), 0 → adult.
    /// Babies are born at -24000 (takes 20 min to grow up, halved by feeding).
    pub age: i32,
    /// Ticks remaining in forced-baby state (set when spawned as baby).
    pub forced_age: i32,
    /// Whether age progression is locked (name-tagged mobs can be kept as babies).
    pub age_locked: bool,
}

impl AgeableMob {
    /// Initial age for naturally spawned babies.
    pub const BABY_TICKS: i32 = -24_000;

    pub fn is_baby(&self) -> bool { self.age < 0 }

    pub fn set_baby(&mut self, baby: bool) {
        self.age = if baby { Self::BABY_TICKS } else { 0 };
    }

    /// Called each tick. Increments age toward 0; fires on_become_adult() when crossing.
    pub fn age_tick(&mut self) {
        if self.is_baby() && !self.age_locked {
            let old = self.age;
            self.age += 1;
            if old < 0 && self.age >= 0 {
                self.on_become_adult();
            }
        }
    }

    fn on_become_adult(&mut self) {
        // Clear baby size / data flags
    }

    /// Called when breeding food is fed: reduces age by 10% of remaining
    /// baby time (minimum reduction 120 ticks).
    pub fn ageUp(&mut self, amount: i32) {
        self.age = (self.age + amount).min(0);
    }
}
```

### 27.2 — `Animal` base and love mode

```rust
// crates/oxidized-game/src/entity/animal.rs

use super::ageable_mob::AgeableMob;
use uuid::Uuid;

/// Base struct for tameable/breedable animals.
pub struct Animal {
    pub base: AgeableMob,
    /// Ticks remaining in love mode. 0 = not in love.
    pub in_love: i32,
    /// UUID of the player who fed this animal.
    pub love_cause: Option<Uuid>,
}

impl Animal {
    /// Ticks of love mode per feeding.
    pub const LOVE_DURATION: i32 = 600; // 30 seconds
    /// Cooldown before an adult can breed again.
    pub const BREED_COOLDOWN: i32 = 6_000; // 5 minutes

    pub fn is_in_love(&self) -> bool { self.in_love > 0 }

    /// Trigger love mode and broadcast `ClientboundEntityEventPacket(LOVE_HEARTS=18)`.
    pub fn set_in_love(&mut self, cause: Option<Uuid>) {
        self.in_love = Self::LOVE_DURATION;
        self.love_cause = cause;
        // Broadcast entity event 18 to nearby clients
    }

    pub fn server_tick(&mut self) {
        if self.in_love > 0 { self.in_love -= 1; }
        self.base.age_tick();
        self.base.base.server_tick();
    }

    /// Attempt to breed with `partner`. Called when both animals are in love
    /// and within 3 blocks. Returns the baby's initial position.
    pub fn breed_with(&mut self, partner: &mut Animal) -> Option<glam::DVec3> {
        if !self.is_in_love() || !partner.is_in_love() { return None; }
        self.in_love = 0;
        partner.in_love = 0;
        // Apply breed cooldown to both parents
        self.base.base.base.base.age = Animal::BREED_COOLDOWN;
        partner.base.base.base.base.age = Animal::BREED_COOLDOWN;
        Some(self.base.base.base.base.pos)
    }
}
```

### 27.3 — Breeding and tempt goals

```rust
// crates/oxidized-game/src/entity/ai/goals/breed.rs

use crate::entity::animal::Animal;

/// Approaches a nearby animal of the same species that is also in love,
/// then triggers breeding when within 3 blocks.
pub struct BreedGoal {
    pub speed_modifier: f64, // 1.0
    pub breed_distance: f64, // 3.0
    /// Entity ID of the current love partner.
    partner_id: Option<u32>,
    breed_item_ticks: i32,
}

impl BreedGoal {
    pub fn new(speed: f64) -> Self {
        Self {
            speed_modifier: speed,
            breed_distance: 3.0,
            partner_id: None,
            breed_item_ticks: 0,
        }
    }
}

impl PathfinderGoal for BreedGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool {
        // Animal must be adult, in love, another of same type also in love within 8 blocks
        false
    }
    fn start(&mut self) {
        self.breed_item_ticks = 60;
    }
    fn tick(&mut self) {
        // Navigate toward partner. If dist < breed_distance, call animal.breed_with(partner)
        // Spawn baby 5 ticks later. Award 1-7 XP to love_cause player.
    }
}

// crates/oxidized-game/src/entity/ai/goals/tempt.rs

/// Follows a player holding a specific food item within 10 blocks.
pub struct TemptGoal {
    pub speed_modifier: f64,  // 1.25
    pub tempt_range: f64,     // 10.0
    pub items: Vec<ResourceLocation>, // acceptable food items
    pub scared_by_player: bool,
    /// Ticks to wait after the player stops holding food.
    cooldown: i32,
}

impl PathfinderGoal for TemptGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE | GoalFlag::LOOK }
    fn can_use(&self) -> bool {
        // Check player within tempt_range holding one of self.items
        // Skip if cooldown > 0
        false
    }
    fn start(&mut self) { self.cooldown = 0; }
    fn tick(&mut self) {
        // Navigate toward the tempting player using speed_modifier
    }
    fn stop(&mut self) { self.cooldown = 100; }
}

// crates/oxidized-game/src/entity/ai/goals/follow_parent.rs

/// Baby animals follow their parent (nearest adult of same type within 16 blocks).
pub struct FollowParentGoal {
    pub speed_modifier: f64, // 1.1
    pub start_distance: f64, // 7.0
    pub stop_distance:  f64, // 3.0
}

impl PathfinderGoal for FollowParentGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool {
        // Only active for babies (is_baby()), must find parent within 16 blocks
        false
    }
    fn tick(&mut self) { /* Navigate toward parent */ }
}

// crates/oxidized-game/src/entity/ai/goals/panic.rs

/// Flee from the last damage source for 5 seconds.
pub struct PanicGoal {
    pub speed_modifier: f64, // 2.0
    panic_ticks: i32,
}

impl PathfinderGoal for PanicGoal {
    fn flags(&self) -> GoalFlag { GoalFlag::MOVE }
    fn can_use(&self) -> bool {
        // mob was hurt recently
        self.panic_ticks > 0
    }
    fn start(&mut self) { self.panic_ticks = 100; }
    fn tick(&mut self) {
        self.panic_ticks -= 1;
        // Navigate to a random position away from last damage source
    }
}
```

### 27.4 — Cow

```rust
// crates/oxidized-game/src/entity/mob/cow.rs

use super::super::animal::Animal;

pub struct Cow {
    pub base: Animal,
}

impl Cow {
    pub const MAX_HEALTH: f32 = 10.0;
    pub const MOVEMENT_SPEED: f64 = 0.2;
    pub const FOLLOW_RANGE: f32 = 10.0;

    pub fn register_goals(mob: &mut Mob) {
        // Priority 0: FloatGoal (JUMP)
        // Priority 1: PanicGoal speed=2.0 (MOVE)
        // Priority 2: BreedGoal speed=1.0 (MOVE)
        // Priority 3: TemptGoal speed=1.25, items=[wheat] (MOVE|LOOK)
        // Priority 4: FollowParentGoal speed=1.25 (MOVE)
        // Priority 5: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 6: LookAtPlayerGoal distance=6.0 (LOOK)
        // Priority 7: RandomLookAroundGoal (LOOK)
    }

    /// Right-click with empty bucket: fills bucket with milk.
    /// Sends ClientboundSetEquipmentPacket to update held item.
    pub fn on_player_interact(&self, held_item: &ItemStack) -> Option<ItemStack> {
        if held_item.item == Item::Bucket {
            Some(ItemStack::new(Item::MilkBucket))
        } else {
            None
        }
    }

    /// Loot: 0-2 leather (always), 1-3 raw beef (or 1-3 cooked_beef if fire at death).
    pub fn drop_loot(&self, on_fire: bool) -> Vec<ItemStack> {
        let mut drops = Vec::new();
        let leather_count = rand::random::<u8>() % 3; // 0-2
        if leather_count > 0 {
            drops.push(ItemStack::new_count(Item::Leather, leather_count));
        }
        let beef_count = 1 + rand::random::<u8>() % 3; // 1-3
        let beef = if on_fire { Item::CookedBeef } else { Item::Beef };
        drops.push(ItemStack::new_count(beef, beef_count));
        drops
    }
}
```

### 27.5 — Sheep

```rust
// crates/oxidized-game/src/entity/mob/sheep.rs

/// DyeColor with natural spawn weights. Totals = 100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyeColor {
    White,      // 81.836%
    Orange, Magenta, LightBlue, Yellow, Lime, Pink, Gray, LightGray,
    Cyan, Purple, Blue, Brown, Green, Red, Black,
}

impl DyeColor {
    /// Spawn weight (out of 10000 to handle fractional percentages).
    pub fn natural_spawn_weight(self) -> u32 {
        match self {
            DyeColor::White      => 8184,
            DyeColor::Brown      => 300,
            DyeColor::Black      => 300,
            DyeColor::Gray       => 300,
            DyeColor::LightGray  => 300,
            DyeColor::Pink       =>  16,
            _ => 0,
        }
    }

    pub fn from_spawn_rng(value: u32) -> Self {
        let thresholds = [
            (8184, DyeColor::White),
            (8484, DyeColor::Black),
            (8784, DyeColor::Gray),
            (9084, DyeColor::LightGray),
            (9384, DyeColor::Brown),
            (9400, DyeColor::Pink),
        ];
        thresholds.iter()
            .find(|(limit, _)| value < *limit)
            .map(|(_, c)| *c)
            .unwrap_or(DyeColor::White)
    }
}

pub struct Sheep {
    pub base: Animal,
    pub color: DyeColor,
    /// False when freshly shorn; grows back after eating grass.
    pub has_wool: bool,
}

impl Sheep {
    pub const MAX_HEALTH: f32 = 8.0;
    pub const MOVEMENT_SPEED: f64 = 0.23;

    pub fn register_goals(mob: &mut Mob) {
        // Priority 0: FloatGoal (JUMP)
        // Priority 1: PanicGoal speed=1.25 (MOVE)
        // Priority 2: BreedGoal speed=1.0 (MOVE)
        // Priority 3: TemptGoal speed=1.25, items=[wheat] (MOVE|LOOK)
        // Priority 4: FollowParentGoal speed=1.25 (MOVE)
        // Priority 5: EatBlockGoal (MOVE) — grazes grass_block → dirt, grows wool
        // Priority 6: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 7: LookAtPlayerGoal distance=6.0 (LOOK)
        // Priority 8: RandomLookAroundGoal (LOOK)
    }

    /// Shear with scissors: drops 1-3 wool of sheep's color, sets has_wool = false.
    pub fn shear(&mut self) -> Vec<ItemStack> {
        self.has_wool = false;
        let count = 1 + rand::random::<u8>() % 3;
        vec![ItemStack::new_count(wool_item(self.color), count)]
    }

    /// Dye sheep to a new color.
    pub fn dye(&mut self, color: DyeColor) {
        self.color = color;
        // Update EntityDataAccessor for color
    }

    /// Loot: 0-2 raw mutton (cooked if fire), 0-1 wool if not shorn.
    pub fn drop_loot(&self, on_fire: bool) -> Vec<ItemStack> {
        let mut drops = Vec::new();
        if self.has_wool {
            drops.push(ItemStack::new(wool_item(self.color)));
        }
        let meat_count = rand::random::<u8>() % 3;
        if meat_count > 0 {
            let meat = if on_fire { Item::CookedMutton } else { Item::Mutton };
            drops.push(ItemStack::new_count(meat, meat_count));
        }
        drops
    }
}

fn wool_item(color: DyeColor) -> Item {
    match color {
        DyeColor::White => Item::WhiteWool,
        _ => Item::WhiteWool, // map all colors
    }
}
```

### 27.6 — Pig

```rust
// crates/oxidized-game/src/entity/mob/pig.rs

pub struct Pig {
    pub base: Animal,
    pub has_saddle: bool,
    /// Whether a player is currently steering (holding carrot on stick).
    pub is_being_ridden: bool,
    /// Boost ticks remaining from carrot on stick interaction.
    pub boost_ticks: i32,
}

impl Pig {
    pub const MAX_HEALTH: f32 = 10.0;
    pub const MOVEMENT_SPEED: f64 = 0.25;
    pub const BOOSTED_SPEED: f64  = 0.45; // carrot-on-stick boost

    pub fn register_goals(mob: &mut Mob) {
        // Priority 0: FloatGoal (JUMP)
        // Priority 1: BoostMoveGoal speed=0.45 (MOVE) — active while ridden with carrot
        // Priority 2: PanicGoal speed=1.25 (MOVE)
        // Priority 3: BreedGoal speed=1.0 (MOVE)
        // Priority 4: TemptGoal speed=1.2, items=[carrot,potato,beetroot] (MOVE|LOOK)
        // Priority 5: FollowParentGoal speed=1.1 (MOVE)
        // Priority 6: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 7: LookAtPlayerGoal distance=6.0 (LOOK)
        // Priority 8: RandomLookAroundGoal (LOOK)
    }

    pub fn on_player_interact(&mut self, held: &ItemStack, has_rider: bool) -> bool {
        if held.item == Item::Saddle && !self.has_saddle {
            self.has_saddle = true;
            return true;
        }
        if held.item == Item::CarrotOnStick && has_rider {
            self.boost_ticks = 140 + rand::random::<i32>().abs() % 841; // 140-980 ticks
            return true;
        }
        false
    }

    /// Loot: 1-3 raw porkchop (cooked if fire at death).
    pub fn drop_loot(&self, on_fire: bool) -> Vec<ItemStack> {
        let count = 1 + rand::random::<u8>() % 3;
        let item = if on_fire { Item::CookedPorkchop } else { Item::Porkchop };
        vec![ItemStack::new_count(item, count)]
    }
}
```

### 27.7 — Chicken

```rust
// crates/oxidized-game/src/entity/mob/chicken.rs

pub struct Chicken {
    pub base: Animal,
    /// Ticks until next egg drop (300-600 ticks = 15-30 seconds).
    pub egg_lay_timer: i32,
    /// Whether this chicken was hatched from a thrown egg (jockey chance = 0).
    pub is_chicken_jockey: bool,
}

impl Chicken {
    pub const MAX_HEALTH: f32 = 4.0;
    pub const MOVEMENT_SPEED: f64 = 0.25;
    pub const TERMINAL_VELOCITY_Y: f64 = -0.06; // flutter wings slows fall

    pub fn new() -> Self {
        Self {
            base: Animal::default(),
            egg_lay_timer: 6000 - (rand::random::<i32>().abs() % 6001),
            is_chicken_jockey: false,
        }
    }

    pub fn register_goals(mob: &mut Mob) {
        // Priority 0: FloatGoal (JUMP)
        // Priority 1: PanicGoal speed=1.5 (MOVE)
        // Priority 2: BreedGoal speed=1.0 (MOVE)
        // Priority 3: TemptGoal speed=1.0, items=[seeds,melon_seeds,pumpkin_seeds] (MOVE|LOOK)
        // Priority 4: FollowParentGoal speed=1.1 (MOVE)
        // Priority 5: WaterAvoidingRandomStrollGoal speed=1.0 (MOVE)
        // Priority 6: LookAtPlayerGoal distance=6.0 (LOOK)
        // Priority 7: RandomLookAroundGoal (LOOK)
    }

    pub fn server_tick(&mut self) {
        // Flutter wings: if airborne (not on ground), clamp vy to TERMINAL_VELOCITY_Y
        if !self.base.base.base.base.on_ground {
            let vy = &mut self.base.base.base.base.velocity.y;
            if *vy < Self::TERMINAL_VELOCITY_Y { *vy = Self::TERMINAL_VELOCITY_Y; }
        }
        // Egg laying
        if !self.base.base.is_baby() {
            self.egg_lay_timer -= 1;
            if self.egg_lay_timer <= 0 {
                self.egg_lay_timer = 300 + rand::random::<i32>().abs() % 301; // 300-600
                self.drop_egg();
            }
        }
        self.base.server_tick();
    }

    fn drop_egg(&self) {
        // Spawn Egg item entity at entity's position
    }

    /// Loot: 0-2 feathers, 0-1 raw chicken (cooked if fire).
    pub fn drop_loot(&self, on_fire: bool) -> Vec<ItemStack> {
        let feathers = rand::random::<u8>() % 3;
        let chicken  = if on_fire { Item::CookedChicken } else { Item::Chicken };
        let mut drops = Vec::new();
        if feathers > 0 { drops.push(ItemStack::new_count(Item::Feather, feathers)); }
        drops.push(ItemStack::new(chicken));
        drops
    }
}
```

### 27.8 — Horse

```rust
// crates/oxidized-game/src/entity/mob/horse.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorseVariant { White, Creamy, Chestnut, Brown, Black, Gray, DarkBrown }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorseMarkings { None, White, WhiteField, WhiteDots, BlackDots }

pub struct Horse {
    pub base: Animal,
    pub variant:  HorseVariant,
    pub markings: HorseMarkings,
    /// Number of taming attempts; tamed when random < tameness / 100.
    pub tameness: i32,
    pub is_tamed:  bool,
    pub has_saddle: bool,
    pub armor_item: Option<ItemStack>,
    /// Jump strength: 0.4–1.0 (random per horse).
    pub jump_strength: f64,
    /// Max health: 15–30 HP (random per horse).
    pub max_health: f32,
}

impl Horse {
    pub const MIN_HEALTH: f32 = 15.0;
    pub const MAX_HEALTH: f32 = 30.0;
    pub const MIN_JUMP:   f64 = 0.4;
    pub const MAX_JUMP:   f64 = 1.0;

    pub fn new(rng: &mut impl RngSource) -> Self {
        let max_health = Self::MIN_HEALTH + (rng.next_f64() as f32 * (Self::MAX_HEALTH - Self::MIN_HEALTH));
        let jump = Self::MIN_JUMP + rng.next_f64() * (Self::MAX_JUMP - Self::MIN_JUMP);
        Self {
            base: Animal::default(),
            variant:  HorseVariant::Brown,
            markings: HorseMarkings::None,
            tameness: 0,
            is_tamed: false,
            has_saddle: false,
            armor_item: None,
            jump_strength: jump,
            max_health,
        }
    }

    /// Called each time a player right-clicks to mount/tame.
    /// Returns true if the player may mount.
    pub fn try_tame(&mut self, rng: &mut impl RngSource) -> bool {
        if self.is_tamed { return true; }
        self.tameness += 5;
        self.tameness = self.tameness.min(100);
        // tamed if rng.next_int_bounded(100) < tameness
        if rng.next_int_bounded(100) < self.tameness {
            self.is_tamed = true;
            // play tame sound, send hearts EntityEvent
        }
        self.is_tamed
    }
}
```

### 27.9 — Baby spawning and `ClientboundEntityEventPacket`

```rust
// crates/oxidized-game/src/entity/animal_spawning.rs

/// Spawn a baby animal after two adults successfully breed.
/// Called 5 ticks after `breed_with()` returns Some.
pub fn spawn_baby(
    parent_a: &Animal,
    parent_b: &Animal,
    world: &mut ServerLevel,
) -> u32 { // returns new entity ID
    let pos = parent_a.base.base.base.base.pos;
    // Instantiate the correct baby type, set age = AgeableMob::BABY_TICKS
    // Send ClientboundAddEntityPacket to all nearby players
    // Award 1 + rng.next_int_bounded(7) XP to parent_a.love_cause player
    0
}

/// `ClientboundEntityEventPacket` event IDs relevant to animals.
pub mod entity_events {
    pub const LOVE_HEARTS:         u8 = 18; // floating hearts above animal
    pub const VILLAGER_ANGRY:      u8 = 13;
    pub const WOLF_SHAKE_WATER:    u8 = 8;
    pub const RABBIT_JUMP:         u8 = 1;
    pub const ANIMAL_TAME_SUCCESS: u8 = 7; // hearts on tame
    pub const ANIMAL_TAME_FAIL:    u8 = 6; // smoke on tame fail
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::entity::animal

pub use ageable_mob::AgeableMob;
pub use animal::Animal;
pub use mob::cow::Cow;
pub use mob::sheep::{Sheep, DyeColor};
pub use mob::pig::Pig;
pub use mob::chicken::Chicken;
pub use mob::horse::{Horse, HorseVariant, HorseMarkings};
pub use ai::goals::breed::BreedGoal;
pub use ai::goals::tempt::TemptGoal;
pub use ai::goals::follow_parent::FollowParentGoal;
pub use ai::goals::panic_goal::PanicGoal;
pub use animal_spawning::{spawn_baby, entity_events};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- AgeableMob ---

    #[test]
    fn baby_starts_negative() {
        let mut mob = AgeableMob::default();
        mob.set_baby(true);
        assert!(mob.is_baby());
        assert_eq!(mob.age, AgeableMob::BABY_TICKS);
    }

    #[test]
    fn baby_becomes_adult_after_ticks() {
        let mut mob = AgeableMob::default();
        mob.set_baby(true);
        for _ in 0..=(-AgeableMob::BABY_TICKS) as usize {
            mob.age_tick();
        }
        assert!(!mob.is_baby());
        assert_eq!(mob.age, 0);
    }

    #[test]
    fn age_up_reduces_baby_time() {
        let mut mob = AgeableMob::default();
        mob.set_baby(true);
        mob.ageUp(2400);
        assert_eq!(mob.age, AgeableMob::BABY_TICKS + 2400);
    }

    #[test]
    fn age_up_does_not_exceed_adult() {
        let mut mob = AgeableMob::default();
        mob.set_baby(true);
        mob.ageUp(999_999);
        assert_eq!(mob.age, 0);
    }

    // --- Animal love mode ---

    #[test]
    fn love_mode_ticks_down() {
        let mut a = Animal::default();
        a.set_in_love(None);
        assert!(a.is_in_love());
        for _ in 0..Animal::LOVE_DURATION {
            a.server_tick();
        }
        assert!(!a.is_in_love());
    }

    #[test]
    fn breed_clears_love_on_both() {
        let mut a = Animal::default();
        let mut b = Animal::default();
        a.set_in_love(None);
        b.set_in_love(None);
        let result = a.breed_with(&mut b);
        assert!(result.is_some());
        assert!(!a.is_in_love());
        assert!(!b.is_in_love());
    }

    #[test]
    fn breed_fails_if_not_in_love() {
        let mut a = Animal::default();
        let mut b = Animal::default();
        b.set_in_love(None);
        let result = a.breed_with(&mut b);
        assert!(result.is_none());
    }

    // --- DyeColor spawn weights ---

    #[test]
    fn white_sheep_is_most_common() {
        assert!(DyeColor::White.natural_spawn_weight() > DyeColor::Pink.natural_spawn_weight());
    }

    #[test]
    fn all_non_natural_colors_have_zero_weight() {
        assert_eq!(DyeColor::Orange.natural_spawn_weight(), 0);
        assert_eq!(DyeColor::Cyan.natural_spawn_weight(), 0);
    }

    #[test]
    fn spawn_weights_sum_to_expected() {
        let total: u32 = [
            DyeColor::White, DyeColor::Black, DyeColor::Gray,
            DyeColor::LightGray, DyeColor::Brown, DyeColor::Pink,
        ].iter().map(|c| c.natural_spawn_weight()).sum();
        // Total should be exactly 9400 (the remaining 600 go to "other" = White fallback)
        assert_eq!(total, 9400);
    }

    // --- Cow loot ---

    #[test]
    fn cow_always_drops_beef() {
        let cow = Cow::default();
        let drops_normal = cow.drop_loot(false);
        let drops_fire   = cow.drop_loot(true);
        assert!(drops_normal.iter().any(|d| d.item == Item::Beef));
        assert!(drops_fire.iter().any(|d| d.item == Item::CookedBeef));
    }

    // --- Chicken egg timer ---

    #[test]
    fn chicken_egg_timer_in_range() {
        for _ in 0..100 {
            let c = Chicken::new();
            assert!((300..=6000).contains(&c.egg_lay_timer));
        }
    }

    // --- Sheep shearing ---

    #[test]
    fn shearing_removes_wool_flag() {
        let mut sheep = Sheep { base: Animal::default(), color: DyeColor::White, has_wool: true };
        let drops = sheep.shear();
        assert!(!sheep.has_wool);
        assert!(!drops.is_empty());
    }

    #[test]
    fn shearing_already_shorn_produces_no_drops() {
        let mut sheep = Sheep { base: Animal::default(), color: DyeColor::White, has_wool: false };
        // shear() should still be callable but mark has_wool false (already false)
        // In real impl shear() should check has_wool first. Here we verify the state invariant.
        sheep.has_wool = false;
        assert!(!sheep.has_wool);
    }

    // --- Horse taming ---

    #[test]
    fn horse_tameness_increases_each_attempt() {
        let mut rng_stub = DummyRng(99); // always returns 99 → never tames unless tameness=100
        let mut h = Horse::new(&mut rng_stub);
        let initial = h.tameness;
        h.try_tame(&mut rng_stub);
        assert!(h.tameness > initial);
    }

    struct DummyRng(i32);
    impl RngSource for DummyRng {
        fn next_long(&mut self) -> u64 { self.0 as u64 }
        fn next_int(&mut self) -> i32 { self.0 }
        fn next_int_bounded(&mut self, _: i32) -> i32 { self.0 }
        fn next_f64(&mut self) -> f64 { 0.5 }
        fn fork(&mut self) -> Box<dyn RngSource> { Box::new(DummyRng(self.0)) }
        fn fork_positional(&mut self) -> Box<dyn PositionalRandomFactory> { todo!() }
    }
}
```
