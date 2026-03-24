# Phase 31 — Advancements

**Status:** 📋 Planned  
**Crate:** `oxidized-game`  
**Reward:** Advancements pop up when unlocked, progress tracked per-player, `/advancement` works.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-011: Registry System](../adr/adr-011-registry-system.md) — compiled core registries + runtime data-driven content


## Goal

Implement the full advancement system: load advancement JSON trees from the data
pack, dispatch criterion triggers on game events (killing mobs, picking up items,
changing location, etc.), track per-player progress, grant rewards (XP, recipes,
loot, functions), send `ClientboundUpdateAdvancementsPacket` for display and
toast notifications, and implement the `/advancement` command with all five
sub-commands. Advancements must persist to and load from player data NBT.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Advancement tree | `AdvancementTree` | `net.minecraft.advancements.AdvancementTree` |
| Server advancement manager | `ServerAdvancementManager` | `net.minecraft.server.ServerAdvancementManager` |
| Player advancements | `ServerPlayerAdvancements` | `net.minecraft.server.PlayerAdvancements` |
| Advancement progress | `AdvancementProgress` | `net.minecraft.advancements.AdvancementProgress` |
| Criterion trigger (trait) | `CriterionTrigger` | `net.minecraft.advancements.critereon.CriterionTrigger` |
| Inventory changed trigger | `InventoryChangeTrigger` | `net.minecraft.advancements.critereon.InventoryChangeTrigger` |
| Location trigger | `LocationTrigger` | `net.minecraft.advancements.critereon.LocationTrigger` |
| Kill entity trigger | `KilledTrigger` | `net.minecraft.advancements.critereon.KilledTrigger` |
| Update advancements packet | `ClientboundUpdateAdvancementsPacket` | `net.minecraft.network.protocol.game.ClientboundUpdateAdvancementsPacket` |

---

## Tasks

### 31.1 — Advancement JSON schema and in-memory representation

```rust
// crates/oxidized-game/src/advancement/mod.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Advancement JSON file (`data/<ns>/advancement/<path>.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct AdvancementJson {
    pub parent:       Option<ResourceLocation>,
    pub display:      Option<AdvancementDisplay>,
    pub criteria:     HashMap<String, CriterionJson>,
    /// Requirements: outer AND, inner OR over criterion names.
    pub requirements: Option<Vec<Vec<String>>>,
    pub rewards:      Option<AdvancementRewards>,
    pub sends_telemetry_event: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdvancementDisplay {
    pub title:           serde_json::Value, // JSON text component
    pub description:     serde_json::Value,
    pub icon:            AdvancementIcon,
    pub frame:           Option<AdvancementFrame>,
    pub background:      Option<ResourceLocation>,
    pub show_toast:      Option<bool>,
    pub announce_to_chat: Option<bool>,
    pub hidden:          Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdvancementIcon {
    pub id:   ResourceLocation,
    pub components: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdvancementFrame { Task, Challenge, Goal }

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdvancementRewards {
    pub experience: Option<i32>,
    pub recipes:    Option<Vec<ResourceLocation>>,
    pub loot:       Option<Vec<ResourceLocation>>,
    pub function:   Option<ResourceLocation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CriterionJson {
    pub trigger:    ResourceLocation,
    pub conditions: Option<serde_json::Value>,
}

/// Compiled advancement node in the server's tree.
#[derive(Debug, Clone)]
pub struct Advancement {
    pub id:           ResourceLocation,
    pub parent:       Option<ResourceLocation>,
    pub display:      Option<AdvancementDisplay>,
    pub criteria:     Vec<String>,
    /// requirements[i] = OR group of criterion names; all groups must be satisfied.
    pub requirements: Vec<Vec<String>>,
    pub rewards:      AdvancementRewards,
}

impl Advancement {
    pub fn from_json(id: ResourceLocation, json: AdvancementJson) -> Self {
        let criteria: Vec<String> = json.criteria.keys().cloned().collect();
        let requirements = json.requirements.unwrap_or_else(|| {
            // Default: each criterion is its own AND group (all must be satisfied).
            criteria.iter().map(|c| vec![c.clone()]).collect()
        });
        Self {
            id,
            parent: json.parent,
            display: json.display,
            criteria,
            requirements,
            rewards: json.rewards.unwrap_or_default(),
        }
    }

    /// Returns true when all requirement groups are satisfied.
    pub fn is_done(&self, progress: &AdvancementProgress) -> bool {
        self.requirements.iter().all(|group| {
            group.iter().any(|name| progress.criterion_done(name))
        })
    }
}
```

### 31.2 — `AdvancementProgress`

```rust
// crates/oxidized-game/src/advancement/progress.rs

use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Per-player progress state for a single advancement.
#[derive(Debug, Clone, Default)]
pub struct AdvancementProgress {
    /// Criterion name → completion timestamp (None = not yet done).
    pub criteria: HashMap<String, Option<DateTime<Utc>>>,
}

impl AdvancementProgress {
    pub fn new(criterion_names: &[String]) -> Self {
        let criteria = criterion_names.iter()
            .map(|n| (n.clone(), None))
            .collect();
        Self { criteria }
    }

    /// Mark a single criterion as complete now. Returns true if newly completed.
    pub fn grant_criterion(&mut self, name: &str) -> bool {
        match self.criteria.get_mut(name) {
            Some(entry @ None) => {
                *entry = Some(Utc::now());
                true
            }
            _ => false, // unknown criterion or already granted
        }
    }

    /// Revoke a criterion. Returns true if it was previously granted.
    pub fn revoke_criterion(&mut self, name: &str) -> bool {
        match self.criteria.get_mut(name) {
            Some(entry @ Some(_)) => {
                *entry = None;
                true
            }
            _ => false,
        }
    }

    pub fn criterion_done(&self, name: &str) -> bool {
        self.criteria.get(name).and_then(|v| *v).is_some()
    }

    /// Whether all registered criteria have been completed.
    pub fn is_complete(&self) -> bool {
        self.criteria.values().all(|v| v.is_some())
    }

    pub fn count_done(&self) -> usize {
        self.criteria.values().filter(|v| v.is_some()).count()
    }

    pub fn count_total(&self) -> usize {
        self.criteria.len()
    }
}
```

### 31.3 — `CriterionTrigger` trait and built-in triggers

```rust
// crates/oxidized-game/src/advancement/trigger.rs

use uuid::Uuid;

/// Context passed to a trigger when a game event fires.
pub struct TriggerContext<'a> {
    pub player_id: Uuid,
    pub world:     &'a dyn WorldView,
}

/// Trait for all criterion triggers.
pub trait CriterionTrigger: Send + Sync {
    fn trigger_id(&self) -> &'static ResourceLocation;

    /// Attempt to fire this trigger given `ctx`. Returns the set of criterion names
    /// that now pass (may be empty if conditions are not met).
    fn test_trigger(&self, ctx: &TriggerContext<'_>, conditions: &serde_json::Value) -> bool;

    /// Called by the advancement engine to check listeners after a game event.
    fn trigger(
        &self,
        ctx:        &TriggerContext<'_>,
        listeners:  &[(ResourceLocation, String, serde_json::Value)], // (adv_id, criterion_name, conditions)
    ) -> Vec<(ResourceLocation, String)> {
        listeners.iter()
            .filter(|(_, _, cond)| self.test_trigger(ctx, cond))
            .map(|(adv, crit, _)| (adv.clone(), crit.clone()))
            .collect()
    }
}

// --- Built-in trigger implementations ---

/// `minecraft:impossible` — never fires; used for manually granted advancements.
pub struct ImpossibleTrigger;
impl CriterionTrigger for ImpossibleTrigger {
    fn trigger_id(&self) -> &'static ResourceLocation { static ID: std::sync::LazyLock<ResourceLocation> = std::sync::LazyLock::new(|| ResourceLocation::new("minecraft:impossible")); &ID }
    fn test_trigger(&self, _: &TriggerContext<'_>, _: &serde_json::Value) -> bool { false }
}

/// `minecraft:inventory_changed` — fires when the player's inventory changes.
pub struct InventoryChangedTrigger;
impl CriterionTrigger for InventoryChangedTrigger {
    fn trigger_id(&self) -> &'static ResourceLocation { static ID: std::sync::LazyLock<ResourceLocation> = std::sync::LazyLock::new(|| ResourceLocation::new("minecraft:inventory_changed")); &ID }
    fn test_trigger(&self, _: &TriggerContext<'_>, _conditions: &serde_json::Value) -> bool {
        // Check conditions.items list against player inventory
        true
    }
}

/// `minecraft:player_killed_entity` — fires when the player kills an entity.
pub struct KilledTrigger {
    pub killed_entity: Option<ResourceLocation>,
}
impl CriterionTrigger for KilledTrigger {
    fn trigger_id(&self) -> &'static ResourceLocation { static ID: std::sync::LazyLock<ResourceLocation> = std::sync::LazyLock::new(|| ResourceLocation::new("minecraft:player_killed_entity")); &ID }
    fn test_trigger(&self, _ctx: &TriggerContext<'_>, conditions: &serde_json::Value) -> bool {
        // Check conditions.entity.type against the killed entity type
        true
    }
}

/// `minecraft:location` — fires each tick; checks biome, dimension, position, light.
pub struct LocationTrigger;
impl CriterionTrigger for LocationTrigger {
    fn trigger_id(&self) -> &'static ResourceLocation { static ID: std::sync::LazyLock<ResourceLocation> = std::sync::LazyLock::new(|| ResourceLocation::new("minecraft:location")); &ID }
    fn test_trigger(&self, _: &TriggerContext<'_>, _: &serde_json::Value) -> bool { true }
}

/// `minecraft:recipe_unlocked` — fires when a recipe is added to the player's recipe book.
pub struct RecipeUnlockedTrigger;
impl CriterionTrigger for RecipeUnlockedTrigger {
    fn trigger_id(&self) -> &'static ResourceLocation { static ID: std::sync::LazyLock<ResourceLocation> = std::sync::LazyLock::new(|| ResourceLocation::new("minecraft:recipe_unlocked")); &ID }
    fn test_trigger(&self, _: &TriggerContext<'_>, _: &serde_json::Value) -> bool { true }
}
```

### 31.4 — `ServerPlayerAdvancements`

```rust
// crates/oxidized-game/src/advancement/player_advancements.rs

use std::collections::HashMap;
use super::{Advancement, AdvancementProgress};
use uuid::Uuid;

/// All advancement state for one connected player.
pub struct ServerPlayerAdvancements {
    pub player_id: Uuid,
    /// Progress keyed by advancement ID.
    pub progress:  HashMap<ResourceLocation, AdvancementProgress>,
    /// Advancements granted since the last `ClientboundUpdateAdvancementsPacket`.
    dirty: Vec<ResourceLocation>,
}

impl ServerPlayerAdvancements {
    pub fn new(player_id: Uuid) -> Self {
        Self { player_id, progress: HashMap::new(), dirty: Vec::new() }
    }

    /// Initialize progress slots for a newly loaded advancement tree.
    pub fn init_for_tree(&mut self, tree: &AdvancementTree) {
        for adv in tree.all() {
            self.progress.entry(adv.id.clone())
                .or_insert_with(|| AdvancementProgress::new(&adv.criteria));
        }
    }

    /// Award the given criterion. Returns true if this completion caused
    /// the advancement itself to be granted (all requirements met).
    pub fn award_criterion(
        &mut self,
        advancement: &Advancement,
        criterion:   &str,
    ) -> bool {
        let progress = self.progress
            .entry(advancement.id.clone())
            .or_insert_with(|| AdvancementProgress::new(&advancement.criteria));
        let newly_done = progress.grant_criterion(criterion);
        if newly_done {
            self.dirty.push(advancement.id.clone());
        }
        newly_done && advancement.is_done(progress)
    }

    /// Revoke the given criterion.
    pub fn revoke_criterion(&mut self, advancement: &Advancement, criterion: &str) -> bool {
        if let Some(progress) = self.progress.get_mut(&advancement.id) {
            let changed = progress.revoke_criterion(criterion);
            if changed { self.dirty.push(advancement.id.clone()); }
            return changed;
        }
        false
    }

    /// Build the `ClientboundUpdateAdvancementsPacket` for all dirty advancements
    /// and flush the dirty list.
    pub fn build_update_packet(&mut self, reset: bool) -> ClientboundUpdateAdvancementsPacket {
        let added: Vec<AdvancementProgressEntry> = self.dirty.iter()
            .filter_map(|id| {
                let progress = self.progress.get(id)?;
                Some(AdvancementProgressEntry {
                    id: id.clone(),
                    criteria: progress.criteria.iter()
                        .filter_map(|(name, ts)| {
                            ts.map(|t| CriterionProgress { name: name.clone(), obtained_at: t })
                        })
                        .collect(),
                })
            })
            .collect();
        self.dirty.clear();
        ClientboundUpdateAdvancementsPacket {
            reset,
            added,
            removed: vec![],
        }
    }

    /// Serialize to NBT for player data save.
    pub fn save(&self) -> oxidized_nbt::NbtCompound {
        let mut root = oxidized_nbt::NbtCompound::new();
        for (id, progress) in &self.progress {
            if progress.count_done() == 0 { continue; }
            let mut adv_tag = oxidized_nbt::NbtCompound::new();
            for (crit_name, timestamp) in &progress.criteria {
                if let Some(ts) = timestamp {
                    adv_tag.put_string(crit_name, &ts.to_rfc3339());
                }
            }
            root.put_compound(&id.to_string(), adv_tag);
        }
        root
    }

    /// Load from NBT player data.
    pub fn load(&mut self, root: &oxidized_nbt::NbtCompound) {
        for (id_str, adv_tag) in root.entries() {
            let id = ResourceLocation::new(id_str);
            if let Some(progress) = self.progress.get_mut(&id) {
                if let Some(crit_map) = adv_tag.as_compound() {
                    for (crit_name, ts_nbt) in crit_map.entries() {
                        if let Some(ts_str) = ts_nbt.as_string() {
                            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                                let _ = progress.grant_criterion(crit_name);
                            }
                        }
                    }
                }
            }
        }
    }
}
```

### 31.5 — `AdvancementTree` and `ServerAdvancementManager`

```rust
// crates/oxidized-game/src/advancement/manager.rs

use std::collections::HashMap;
use super::{Advancement, CriterionTrigger};

pub struct AdvancementTree {
    advancements: HashMap<ResourceLocation, Advancement>,
    /// Root advancements (those with no parent).
    roots: Vec<ResourceLocation>,
    /// Listeners: trigger_id → list of (advancement_id, criterion_name, conditions_json).
    listeners: HashMap<ResourceLocation, Vec<(ResourceLocation, String, serde_json::Value)>>,
}

impl AdvancementTree {
    pub fn new() -> Self {
        Self { advancements: HashMap::new(), roots: Vec::new(), listeners: HashMap::new() }
    }

    pub fn register(&mut self, adv: Advancement, criteria_triggers: Vec<(String, ResourceLocation, serde_json::Value)>) {
        if adv.parent.is_none() { self.roots.push(adv.id.clone()); }
        // Register each criterion trigger listener
        for (crit_name, trigger_id, conditions) in criteria_triggers {
            self.listeners.entry(trigger_id)
                .or_default()
                .push((adv.id.clone(), crit_name, conditions));
        }
        self.advancements.insert(adv.id.clone(), adv);
    }

    pub fn get(&self, id: &ResourceLocation) -> Option<&Advancement> {
        self.advancements.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Advancement> {
        self.advancements.values()
    }

    pub fn listeners_for_trigger(&self, trigger_id: &ResourceLocation)
        -> &[(ResourceLocation, String, serde_json::Value)]
    {
        self.listeners.get(trigger_id).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Manages the global advancement tree and dispatches trigger events.
pub struct ServerAdvancementManager {
    pub tree:     AdvancementTree,
    triggers:     HashMap<ResourceLocation, Box<dyn CriterionTrigger>>,
}

impl ServerAdvancementManager {
    pub fn new() -> Self {
        Self { tree: AdvancementTree::new(), triggers: HashMap::new() }
    }

    pub fn register_trigger(&mut self, trigger: impl CriterionTrigger + 'static) {
        self.triggers.insert(trigger.trigger_id().clone(), Box::new(trigger));
    }

    /// Fire a trigger for a specific player. Updates their progress, sends packets.
    pub fn fire_trigger(
        &self,
        trigger_id:  &ResourceLocation,
        ctx:         &TriggerContext<'_>,
        player_advs: &mut ServerPlayerAdvancements,
        rewards_fn:  &mut dyn FnMut(&Advancement, &ServerPlayerAdvancements),
    ) {
        let Some(trigger) = self.triggers.get(trigger_id) else { return };
        let listeners = self.tree.listeners_for_trigger(trigger_id);
        let granted = trigger.trigger(ctx, listeners);
        for (adv_id, crit_name) in granted {
            let Some(adv) = self.tree.get(&adv_id) else { continue };
            let completed = player_advs.award_criterion(adv, &crit_name);
            if completed {
                rewards_fn(adv, player_advs);
            }
        }
    }
}
```

### 31.6 — `ClientboundUpdateAdvancementsPacket` (0x6F)

```rust
// crates/oxidized-game/src/advancement/packets.rs

use chrono::{DateTime, Utc};

/// Sent to the client to add/update/remove advancements and their progress.
/// Packet ID: 0x6F.
#[derive(Debug, Clone)]
pub struct ClientboundUpdateAdvancementsPacket {
    /// If true, the client should reset its advancement tab before applying changes.
    pub reset:   bool,
    /// Advancements whose progress has changed.
    pub added:   Vec<AdvancementProgressEntry>,
    /// Advancement IDs to remove from the client's display.
    pub removed: Vec<ResourceLocation>,
}

#[derive(Debug, Clone)]
pub struct AdvancementProgressEntry {
    pub id:       ResourceLocation,
    pub criteria: Vec<CriterionProgress>,
}

#[derive(Debug, Clone)]
pub struct CriterionProgress {
    pub name:        String,
    pub obtained_at: DateTime<Utc>,
}

impl ClientboundUpdateAdvancementsPacket {
    pub fn encode(&self, buf: &mut Vec<u8>) {
        // bool: reset
        buf.push(self.reset as u8);
        // VarInt: added count
        encode_varint(self.added.len() as i32, buf);
        for entry in &self.added {
            encode_string(&entry.id.to_string(), buf);
            encode_varint(entry.criteria.len() as i32, buf);
            for crit in &entry.criteria {
                encode_string(&crit.name, buf);
                // i64: epoch milliseconds of obtained_at
                let ms = crit.obtained_at.timestamp_millis();
                buf.extend_from_slice(&ms.to_be_bytes());
            }
        }
        // VarInt: removed count
        encode_varint(self.removed.len() as i32, buf);
        for id in &self.removed {
            encode_string(&id.to_string(), buf);
        }
    }
}

fn encode_varint(mut v: i32, buf: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 { buf.push(b); break; } else { buf.push(b | 0x80); }
    }
}

fn encode_string(s: &str, buf: &mut Vec<u8>) {
    encode_varint(s.len() as i32, buf);
    buf.extend_from_slice(s.as_bytes());
}
```

### 31.7 — `/advancement` command

```rust
// crates/oxidized-game/src/commands/advancement.rs

/// /advancement grant|revoke <player> only|from|through|until|everything [<advancement>]
pub enum AdvancementCommand {
    /// Grant or revoke a single advancement and all its criteria.
    Only { grant: bool, player: String, advancement: ResourceLocation },
    /// Grant/revoke the advancement and all ancestors up to the root.
    From { grant: bool, player: String, advancement: ResourceLocation },
    /// Grant/revoke from the given advancement down to all descendants.
    Through { grant: bool, player: String, advancement: ResourceLocation },
    /// Grant/revoke from the root down to (but not including) the given advancement.
    Until { grant: bool, player: String, advancement: ResourceLocation },
    /// Grant/revoke all advancements.
    Everything { grant: bool, player: String },
}

pub fn execute_advancement_command(
    cmd:    AdvancementCommand,
    tree:   &AdvancementTree,
    player: &mut ServerPlayerAdvancements,
) -> CommandResult {
    match cmd {
        AdvancementCommand::Only { grant, advancement, .. } => {
            let Some(adv) = tree.get(&advancement) else {
                return CommandResult::Err(format!("Unknown advancement: {advancement}"));
            };
            let mut count = 0;
            for crit in &adv.criteria {
                if grant { if player.award_criterion(adv, crit)  { count += 1; } }
                else      { if player.revoke_criterion(adv, crit) { count += 1; } }
            }
            CommandResult::Ok(format!("{} {} criterion(s)", if grant { "Granted" } else { "Revoked" }, count))
        }
        AdvancementCommand::Everything { grant, .. } => {
            let advancements: Vec<_> = tree.all().cloned().collect();
            let mut count = 0;
            for adv in &advancements {
                for crit in &adv.criteria {
                    if grant { if player.award_criterion(adv, crit)  { count += 1; } }
                    else      { if player.revoke_criterion(adv, crit) { count += 1; } }
                }
            }
            CommandResult::Ok(format!("{} {} criterion(s)", if grant { "Granted" } else { "Revoked" }, count))
        }
        _ => CommandResult::Ok("Done".to_string()),
    }
}

pub enum CommandResult {
    Ok(String),
    Err(String),
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::advancement

pub use mod::{Advancement, AdvancementJson, AdvancementDisplay, AdvancementFrame, AdvancementRewards};
pub use progress::AdvancementProgress;
pub use trigger::{CriterionTrigger, TriggerContext,
                  ImpossibleTrigger, InventoryChangedTrigger, KilledTrigger,
                  LocationTrigger, RecipeUnlockedTrigger};
pub use player_advancements::ServerPlayerAdvancements;
pub use manager::{AdvancementTree, ServerAdvancementManager};
pub use packets::{ClientboundUpdateAdvancementsPacket, AdvancementProgressEntry, CriterionProgress};
pub use commands::advancement::{AdvancementCommand, execute_advancement_command};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn simple_advancement(id: &str, criteria: &[&str]) -> Advancement {
        Advancement {
            id: rl(id),
            parent: None,
            display: None,
            criteria: criteria.iter().map(|s| s.to_string()).collect(),
            requirements: criteria.iter().map(|s| vec![s.to_string()]).collect(),
            rewards: AdvancementRewards::default(),
        }
    }

    fn rl(s: &str) -> ResourceLocation { ResourceLocation::new(s) }

    // --- AdvancementProgress ---

    #[test]
    fn progress_starts_incomplete() {
        let p = AdvancementProgress::new(&["a".to_string(), "b".to_string()]);
        assert!(!p.is_complete());
        assert_eq!(p.count_done(), 0);
        assert_eq!(p.count_total(), 2);
    }

    #[test]
    fn grant_criterion_marks_done() {
        let mut p = AdvancementProgress::new(&["kill_zombie".to_string()]);
        assert!(p.grant_criterion("kill_zombie"));
        assert!(p.criterion_done("kill_zombie"));
        assert!(p.is_complete());
    }

    #[test]
    fn grant_criterion_twice_returns_false() {
        let mut p = AdvancementProgress::new(&["a".to_string()]);
        assert!(p.grant_criterion("a"));
        assert!(!p.grant_criterion("a")); // second call returns false
    }

    #[test]
    fn revoke_criterion_clears_progress() {
        let mut p = AdvancementProgress::new(&["a".to_string()]);
        p.grant_criterion("a");
        assert!(p.revoke_criterion("a"));
        assert!(!p.criterion_done("a"));
        assert!(!p.is_complete());
    }

    #[test]
    fn unknown_criterion_not_grantable() {
        let mut p = AdvancementProgress::new(&["a".to_string()]);
        // "b" was never registered
        assert!(!p.grant_criterion("b"));
    }

    // --- Advancement.is_done ---

    #[test]
    fn advancement_done_when_all_criteria_met() {
        let adv = simple_advancement("test:kill_things", &["kill_zombie", "kill_skeleton"]);
        let mut progress = AdvancementProgress::new(&adv.criteria);
        progress.grant_criterion("kill_zombie");
        assert!(!adv.is_done(&progress));
        progress.grant_criterion("kill_skeleton");
        assert!(adv.is_done(&progress));
    }

    #[test]
    fn advancement_or_group_passes_with_any_criterion() {
        let adv = Advancement {
            id: rl("test:or_group"),
            parent: None,
            display: None,
            criteria: vec!["a".to_string(), "b".to_string()],
            // Single OR group: either a OR b satisfies the requirement.
            requirements: vec![vec!["a".to_string(), "b".to_string()]],
            rewards: AdvancementRewards::default(),
        };
        let mut progress = AdvancementProgress::new(&adv.criteria);
        progress.grant_criterion("a");
        assert!(adv.is_done(&progress)); // a alone satisfies the OR group
    }

    // --- ServerPlayerAdvancements award/revoke ---

    #[test]
    fn award_criterion_marks_dirty() {
        let adv = simple_advancement("test:single", &["criteria_1"]);
        let mut player = ServerPlayerAdvancements::new(uuid::Uuid::new_v4());
        player.init_for_tree(&{
            let mut tree = AdvancementTree::new();
            tree.register(adv.clone(), vec![]);
            tree
        });
        let completed = player.award_criterion(&adv, "criteria_1");
        assert!(completed);
        // dirty list should contain the advancement
        let packet = player.build_update_packet(false);
        assert_eq!(packet.added.len(), 1);
        assert_eq!(packet.added[0].id, rl("test:single"));
    }

    #[test]
    fn build_update_packet_flushes_dirty() {
        let adv = simple_advancement("test:x", &["c"]);
        let mut player = ServerPlayerAdvancements::new(uuid::Uuid::new_v4());
        player.init_for_tree(&{
            let mut tree = AdvancementTree::new();
            tree.register(adv.clone(), vec![]);
            tree
        });
        player.award_criterion(&adv, "c");
        let _ = player.build_update_packet(false);
        // Second call should have empty added list (dirty was flushed)
        let p2 = player.build_update_packet(false);
        assert!(p2.added.is_empty());
    }

    // --- NBT save/load round-trip ---

    #[test]
    fn player_advancements_nbt_roundtrip() {
        let adv = simple_advancement("minecraft:story/root", &["mine_stone", "kill_mob"]);
        let mut player = ServerPlayerAdvancements::new(uuid::Uuid::new_v4());
        let mut tree = AdvancementTree::new();
        tree.register(adv.clone(), vec![]);
        player.init_for_tree(&tree);
        player.award_criterion(&adv, "mine_stone");

        let saved = player.save();
        let mut loaded = ServerPlayerAdvancements::new(player.player_id);
        loaded.init_for_tree(&tree);
        loaded.load(&saved);

        let progress = &loaded.progress[&rl("minecraft:story/root")];
        assert!(progress.criterion_done("mine_stone"));
        assert!(!progress.criterion_done("kill_mob"));
    }

    // --- Packet encoding ---

    #[test]
    fn update_advancement_packet_encodes_reset_flag() {
        let packet = ClientboundUpdateAdvancementsPacket {
            reset:   true,
            added:   vec![],
            removed: vec![],
        };
        let mut buf = Vec::new();
        packet.encode(&mut buf);
        assert_eq!(buf[0], 1u8); // reset = true
    }

    #[test]
    fn update_advancement_packet_no_reset_encodes_zero() {
        let packet = ClientboundUpdateAdvancementsPacket {
            reset:   false,
            added:   vec![],
            removed: vec![],
        };
        let mut buf = Vec::new();
        packet.encode(&mut buf);
        assert_eq!(buf[0], 0u8);
    }

    // --- /advancement command ---

    #[test]
    fn advancement_command_only_grants_all_criteria() {
        let adv = simple_advancement("test:multi", &["a", "b", "c"]);
        let mut player = ServerPlayerAdvancements::new(uuid::Uuid::new_v4());
        let mut tree = AdvancementTree::new();
        tree.register(adv.clone(), vec![]);
        player.init_for_tree(&tree);

        let cmd = AdvancementCommand::Only {
            grant: true,
            player: "TestPlayer".to_string(),
            advancement: rl("test:multi"),
        };
        let result = execute_advancement_command(cmd, &tree, &mut player);
        assert!(matches!(result, CommandResult::Ok(_)));
        let progress = &player.progress[&rl("test:multi")];
        assert!(progress.is_complete());
    }

    #[test]
    fn advancement_command_revoke_clears_criteria() {
        let adv = simple_advancement("test:rev", &["x"]);
        let mut player = ServerPlayerAdvancements::new(uuid::Uuid::new_v4());
        let mut tree = AdvancementTree::new();
        tree.register(adv.clone(), vec![]);
        player.init_for_tree(&tree);
        player.award_criterion(&adv, "x");

        let cmd = AdvancementCommand::Only {
            grant: false,
            player: "TestPlayer".to_string(),
            advancement: rl("test:rev"),
        };
        execute_advancement_command(cmd, &tree, &mut player);
        assert!(!player.progress[&rl("test:rev")].criterion_done("x"));
    }
}
```
