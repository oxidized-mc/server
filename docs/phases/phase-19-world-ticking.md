# Phase 19 — World Ticking

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** Day/night cycle works, blocks tick, weather changes.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-001: Async Runtime](../adr/adr-001-async-runtime.md) — Tokio runtime selection and async patterns
- [ADR-005: Configuration](../adr/adr-005-configuration.md) — TOML config parsing and validation (superseded by ADR-033)
- [ADR-019: Tick Loop](../adr/adr-019-tick-loop.md) — parallel tick phases with ECS system scheduling


## Goal

Implement the main server tick loop at exactly 20 TPS (one tick every 50 ms),
advance in-game time, send time updates to clients, fire random block ticks and
scheduled ticks, manage weather transitions, and handle overload detection.
Implement `GameRules` and the `/tick` command family for operator control over
tick rate and freeze/step modes.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Main tick loop | `net.minecraft.server.MinecraftServer#tickServer` |
| Level tick method | `net.minecraft.server.level.ServerLevel#tick` |
| Scheduled/block ticks | `net.minecraft.world.ticks.LevelTicks` |
| Tick rate manager | `net.minecraft.server.ServerTickRateManager` |
| Game rules | `net.minecraft.world.level.gamerules.GameRules` |
| Time packet (S→C) | `net.minecraft.network.protocol.game.ClientboundSetTimePacket` |

---

## Tasks

### 19.1 — Tick loop (`oxidized-server/src/tick.rs`)

```rust
use tokio::time::{interval, Duration, Instant, MissedTickBehavior};
use std::sync::Arc;

const TICK_DURATION: Duration = Duration::from_millis(50);
const OVERLOAD_WARN_THRESHOLD: Duration = Duration::from_millis(20);
const OVERLOAD_LOG_THRESHOLD_TICKS: u64 = 20;

pub async fn run_tick_loop(server: Arc<MinecraftServer>) {
    let mut ticker = interval(TICK_DURATION);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut tick_number: u64 = 0;
    let mut behind_ticks: u64 = 0;

    loop {
        let tick_start = Instant::now();
        ticker.tick().await;

        if server.is_shutdown_requested() { break; }

        server.tick(tick_number).await;

        let elapsed = tick_start.elapsed();
        if elapsed > TICK_DURATION + OVERLOAD_WARN_THRESHOLD {
            behind_ticks += 1;
            if behind_ticks >= OVERLOAD_LOG_THRESHOLD_TICKS {
                tracing::warn!(
                    "Can't keep up! Is the server overloaded? Running {}ms or {} ticks behind",
                    elapsed.as_millis() - 50,
                    behind_ticks
                );
                behind_ticks = 0;
            }
        } else {
            behind_ticks = 0;
        }

        tick_number += 1;
    }
}
```

### 19.2 — MinecraftServer::tick (`oxidized-game/src/server/tick.rs`)

```rust
impl MinecraftServer {
    pub async fn tick(&self, tick_number: u64) {
        let tick_mgr = self.tick_rate_manager.read().await;

        if tick_mgr.is_frozen() && !tick_mgr.should_step() {
            return;
        }
        tick_mgr.consume_step();
        drop(tick_mgr);

        // 1. Tick all levels
        for level in self.levels.values() {
            level.write().await.tick(tick_number, &self.game_rules).await;
        }

        // 2. Tick all connected players
        for player in self.players.values() {
            player.write().await.tick(tick_number).await;
        }

        // 3. Auto-save every 6000 ticks (~5 minutes)
        if tick_number % 6000 == 0 && tick_number > 0 {
            self.save_all(false).await;
        }

        // 4. Flush outbound packet queues
        for player in self.players.values() {
            player.write().await.flush_packets().await;
        }
    }
}
```

### 19.3 — ServerLevel::tick (`oxidized-game/src/level/tick.rs`)

```rust
impl ServerLevel {
    pub async fn tick(&mut self, tick_number: u64, rules: &GameRules) {
        // 1. Advance time
        if rules.get_bool(GameRuleKey::DoDaylightCycle) {
            self.data.day_time = (self.data.day_time + 1) % 24000;
            self.data.game_time += 1;
        }

        // 2. Send time to clients every 20 ticks (every real second)
        if tick_number % 20 == 0 {
            self.broadcast_time_packet().await;
        }

        // 3. Weather
        if rules.get_bool(GameRuleKey::DoWeatherCycle) {
            self.tick_weather(rules).await;
        }

        // 4. Random block ticks
        if rules.get_bool(GameRuleKey::DoRandomTicking) {
            let speed = rules.get_int(GameRuleKey::RandomTickSpeed) as usize;
            self.tick_random_blocks(speed);
        }

        // 5. Scheduled ticks (fluid and block)
        self.block_ticks.tick(self.data.game_time, |tick| {
            self.process_block_tick(tick);
        });
        self.fluid_ticks.tick(self.data.game_time, |tick| {
            self.process_fluid_tick(tick);
        });

        // 6. Sleeping detection (skip to dawn)
        self.check_sleeping();
    }

    async fn broadcast_time_packet(&self) {
        let packet = ClientboundSetTimePacket {
            game_time: self.data.game_time,
            day_time: if self.rules.get_bool(GameRuleKey::DoDaylightCycle) {
                self.data.day_time as i64
            } else {
                // Negative value tells client not to advance time on its own
                -(self.data.day_time as i64)
            },
        };
        self.broadcast_packet(packet).await;
    }

    fn tick_random_blocks(&mut self, ticks_per_section: usize) {
        for chunk in self.loaded_chunks.values_mut() {
            for section_y in chunk.min_section_y..=chunk.max_section_y {
                let section = &mut chunk.sections[section_y as usize];
                if section.is_all_air() { continue; }
                for _ in 0..ticks_per_section {
                    let (lx, ly, lz) = random_section_pos(&mut self.random);
                    let block = section.get_block_state(lx, ly, lz);
                    if block.needs_random_tick() {
                        let world_pos = BlockPos::from_section(
                            chunk.pos, section_y, lx, ly, lz,
                        );
                        block.random_tick(self, world_pos, &mut self.random);
                    }
                }
            }
        }
    }

    async fn tick_weather(&mut self, rules: &GameRules) {
        // Count down rain/thunder timers; transition state when they expire
        if self.data.rain_time > 0 {
            self.data.rain_time -= 1;
        } else {
            self.data.raining = !self.data.raining;
            self.data.rain_time = if self.data.raining {
                self.random.next_i32_bounded(12000) + 12000
            } else {
                self.random.next_i32_bounded(168000) + 12000
            };
        }

        if self.data.thunder_time > 0 {
            self.data.thunder_time -= 1;
        } else {
            self.data.thundering = !self.data.thundering;
            self.data.thunder_time = if self.data.thundering {
                self.random.next_i32_bounded(12000) + 3600
            } else {
                self.random.next_i32_bounded(168000) + 12000
            };
        }

        let level_event = self.weather_level_event();
        if let Some(event) = level_event {
            self.broadcast_weather(event).await;
        }
    }
}
```

### 19.4 — ClientboundSetTimePacket

```rust
/// 0x5C – world time update, sent every 20 ticks
#[derive(Debug, Clone)]
pub struct ClientboundSetTimePacket {
    /// Absolute world age (monotonically increasing, never resets)
    pub game_time: i64,
    /// Time of day (0–23999). Negative = frozen (client should not advance).
    pub day_time: i64,
}

impl Encode for ClientboundSetTimePacket {
    fn encode(&self, buf: &mut impl BufMut) -> anyhow::Result<()> {
        self.game_time.encode(buf)?;
        self.day_time.encode(buf)?;
        Ok(())
    }
}
```

### 19.5 — GameRules (`oxidized-game/src/world/game_rules.rs`)

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameRuleKey {
    // Boolean rules
    DoDaylightCycle,
    DoWeatherCycle,
    DoMobSpawning,
    DoMobLoot,
    DoTileDrops,
    DoFireTick,
    DoRandomTicking,
    MobGriefing,
    KeepInventory,
    NaturalRegeneration,
    FallDamage,
    ShowDeathMessages,
    LogAdminCommands,
    CommandBlockOutput,
    SendCommandFeedback,
    DoLimitedCrafting,
    DoEntityDrops,
    Pvp,
    // Integer rules
    RandomTickSpeed,
    SpawnRadius,
    MaxEntityCramming,
    MaxCommandChainLength,
    PlayersNetherPortalDefaultDelay,
    PlayersNetherPortalCreativeDelay,
    UniversalAnger,
}

#[derive(Debug, Clone)]
pub enum GameRuleValue {
    Bool(bool),
    Int(i32),
}

#[derive(Debug, Clone)]
pub struct GameRules {
    values: HashMap<GameRuleKey, GameRuleValue>,
}

impl Default for GameRules {
    fn default() -> Self {
        let mut m = HashMap::new();
        // Boolean defaults
        m.insert(GameRuleKey::DoDaylightCycle,     GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoWeatherCycle,      GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoMobSpawning,       GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoMobLoot,           GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoTileDrops,         GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoFireTick,          GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoRandomTicking,     GameRuleValue::Bool(true));
        m.insert(GameRuleKey::MobGriefing,         GameRuleValue::Bool(true));
        m.insert(GameRuleKey::KeepInventory,       GameRuleValue::Bool(false));
        m.insert(GameRuleKey::NaturalRegeneration, GameRuleValue::Bool(true));
        m.insert(GameRuleKey::FallDamage,          GameRuleValue::Bool(true));
        m.insert(GameRuleKey::ShowDeathMessages,   GameRuleValue::Bool(true));
        m.insert(GameRuleKey::LogAdminCommands,    GameRuleValue::Bool(true));
        m.insert(GameRuleKey::CommandBlockOutput,  GameRuleValue::Bool(true));
        m.insert(GameRuleKey::SendCommandFeedback, GameRuleValue::Bool(true));
        m.insert(GameRuleKey::DoLimitedCrafting,   GameRuleValue::Bool(false));
        m.insert(GameRuleKey::DoEntityDrops,       GameRuleValue::Bool(true));
        m.insert(GameRuleKey::Pvp,                 GameRuleValue::Bool(true));
        // Integer defaults
        m.insert(GameRuleKey::RandomTickSpeed,          GameRuleValue::Int(3));
        m.insert(GameRuleKey::SpawnRadius,              GameRuleValue::Int(8));
        m.insert(GameRuleKey::MaxEntityCramming,        GameRuleValue::Int(24));
        m.insert(GameRuleKey::MaxCommandChainLength,    GameRuleValue::Int(65536));
        m.insert(GameRuleKey::PlayersNetherPortalDefaultDelay, GameRuleValue::Int(80));
        m.insert(GameRuleKey::PlayersNetherPortalCreativeDelay, GameRuleValue::Int(1));
        Self { values: m }
    }
}

impl GameRules {
    pub fn get_bool(&self, key: GameRuleKey) -> bool {
        match self.values.get(&key) {
            Some(GameRuleValue::Bool(v)) => *v,
            _ => false,
        }
    }

    pub fn get_int(&self, key: GameRuleKey) -> i32 {
        match self.values.get(&key) {
            Some(GameRuleValue::Int(v)) => *v,
            _ => 0,
        }
    }

    pub fn set_bool(&mut self, key: GameRuleKey, value: bool) {
        self.values.insert(key, GameRuleValue::Bool(value));
    }

    pub fn set_int(&mut self, key: GameRuleKey, value: i32) {
        self.values.insert(key, GameRuleValue::Int(value));
    }

    /// Name used in NBT and /gamerule command
    pub fn name_of(key: GameRuleKey) -> &'static str {
        match key {
            GameRuleKey::DoDaylightCycle     => "doDaylightCycle",
            GameRuleKey::DoWeatherCycle      => "doWeatherCycle",
            GameRuleKey::DoMobSpawning       => "doMobSpawning",
            GameRuleKey::DoMobLoot           => "doMobLoot",
            GameRuleKey::DoTileDrops         => "doTileDrops",
            GameRuleKey::DoFireTick          => "doFireTick",
            GameRuleKey::DoRandomTicking     => "doRandomTicking",
            GameRuleKey::MobGriefing         => "mobGriefing",
            GameRuleKey::KeepInventory       => "keepInventory",
            GameRuleKey::NaturalRegeneration => "naturalRegeneration",
            GameRuleKey::FallDamage          => "fallDamage",
            GameRuleKey::ShowDeathMessages   => "showDeathMessages",
            GameRuleKey::LogAdminCommands    => "logAdminCommands",
            GameRuleKey::CommandBlockOutput  => "commandBlockOutput",
            GameRuleKey::SendCommandFeedback => "sendCommandFeedback",
            GameRuleKey::DoLimitedCrafting   => "doLimitedCrafting",
            GameRuleKey::DoEntityDrops       => "doEntityDrops",
            GameRuleKey::Pvp                 => "pvp",
            GameRuleKey::RandomTickSpeed     => "randomTickSpeed",
            GameRuleKey::SpawnRadius         => "spawnRadius",
            GameRuleKey::MaxEntityCramming   => "maxEntityCramming",
            GameRuleKey::MaxCommandChainLength => "maxCommandChainLength",
            GameRuleKey::PlayersNetherPortalDefaultDelay => "playersNetherPortalDefaultDelay",
            GameRuleKey::PlayersNetherPortalCreativeDelay => "playersNetherPortalCreativeDelay",
            GameRuleKey::UniversalAnger      => "universalAnger",
        }
    }
}
```

### 19.6 — ServerTickRateManager (`oxidized-game/src/server/tick_rate.rs`)

```rust
pub struct ServerTickRateManager {
    /// Current target TPS (default 20.0)
    pub tick_rate: f32,
    /// Whether the server is frozen (no ticks advance)
    pub frozen: bool,
    /// Remaining steps to advance when frozen
    pub steps_remaining: u32,
    /// Sprint mode: run ticks without sleeping to catch up
    pub sprinting: bool,
    pub sprint_ticks_remaining: u64,
}

impl Default for ServerTickRateManager {
    fn default() -> Self {
        Self {
            tick_rate: 20.0,
            frozen: false,
            steps_remaining: 0,
            sprinting: false,
            sprint_ticks_remaining: 0,
        }
    }
}

impl ServerTickRateManager {
    pub fn tick_interval(&self) -> Duration {
        Duration::from_secs_f32(1.0 / self.tick_rate)
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen && self.steps_remaining == 0
    }

    pub fn should_step(&self) -> bool {
        self.frozen && self.steps_remaining > 0
    }

    pub fn consume_step(&mut self) {
        if self.steps_remaining > 0 {
            self.steps_remaining -= 1;
        }
    }

    pub fn request_steps(&mut self, count: u32) {
        self.steps_remaining += count;
    }
}
```

### 19.7 — `/tick` command family (`oxidized-game/src/commands/tick.rs`)

```rust
pub fn register(d: &mut CommandDispatcher<CommandSourceStack>) {
    d.register(literal("tick")
        .requires(|s| s.has_permission(3))
        .then(literal("query")
            .executes(|ctx| {
                let mgr = ctx.source.get_server().tick_rate_manager.read();
                let status = if mgr.frozen { "frozen" } else { "running" };
                ctx.source.send_success(
                    Component::text(format!("Tick rate: {:.1} TPS, status: {}", mgr.tick_rate, status)),
                    false);
                Ok(1)
            }))
        .then(literal("rate")
            .then(argument("rate", ArgumentType::Float { min: Some(1.0), max: Some(10000.0) })
                .executes(|ctx| {
                    let rate = get_float(&ctx, "rate")?;
                    ctx.source.get_server().tick_rate_manager.write().tick_rate = rate;
                    ctx.source.send_success(
                        Component::text(format!("Tick rate set to {:.1}", rate)), true);
                    Ok(1)
                })))
        .then(literal("freeze")
            .executes(|ctx| {
                ctx.source.get_server().tick_rate_manager.write().frozen = true;
                ctx.source.send_success(Component::text("Server ticking frozen"), true);
                Ok(1)
            }))
        .then(literal("unfreeze")
            .executes(|ctx| {
                let mut mgr = ctx.source.get_server().tick_rate_manager.write();
                mgr.frozen = false;
                mgr.steps_remaining = 0;
                ctx.source.send_success(Component::text("Server ticking resumed"), true);
                Ok(1)
            }))
        .then(literal("step")
            .executes(|ctx| {
                ctx.source.get_server().tick_rate_manager.write().request_steps(1);
                Ok(1)
            })
            .then(argument("ticks", ArgumentType::Integer { min: Some(1), max: None })
                .executes(|ctx| {
                    let n = get_integer(&ctx, "ticks")? as u32;
                    ctx.source.get_server().tick_rate_manager.write().request_steps(n);
                    Ok(1)
                })))
        .then(literal("sprint")
            .then(argument("ticks", ArgumentType::Integer { min: Some(1), max: None })
                .executes(|ctx| {
                    let n = get_integer(&ctx, "ticks")? as u64;
                    let mut mgr = ctx.source.get_server().tick_rate_manager.write();
                    mgr.sprinting = true;
                    mgr.sprint_ticks_remaining = n;
                    Ok(1)
                })))
    );
}
```

---

## Data Structures

```rust
// oxidized-game/src/level/level_data.rs

pub struct LevelData {
    pub game_time: i64,      // absolute tick counter; never resets
    pub day_time: i64,       // 0..23999; 0 = sunrise, 6000 = noon, 13000 = sunset, 18000 = midnight
    pub raining: bool,
    pub rain_time: i32,      // ticks until rain toggles
    pub thundering: bool,
    pub thunder_time: i32,   // ticks until thunder toggles
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub level_name: String,
    pub game_type: GameType,
    pub difficulty: Difficulty,
    pub game_rules: GameRules,
}

// oxidized-game/src/level/scheduled_tick.rs

pub struct ScheduledTick<T> {
    pub pos: BlockPos,
    pub block: T,
    pub trigger_time: i64,   // game_time when tick should fire
    pub priority: TickPriority,
    pub sub_tick: i64,       // ordering within same trigger_time+priority
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TickPriority {
    ExtremelyHigh = -3,
    VeryHigh = -2,
    High = -1,
    Normal = 0,
    Low = 1,
    VeryLow = 2,
    ExtremelyLow = 3,
}

pub struct LevelTicks<T> {
    queue: std::collections::BinaryHeap<std::cmp::Reverse<ScheduledTick<T>>>,
    scheduled: std::collections::HashSet<(BlockPos, T)>,
    current_time: i64,
}

impl<T: Eq + std::hash::Hash + Clone> LevelTicks<T> {
    pub fn schedule(&mut self, pos: BlockPos, block: T, delay: i64, priority: TickPriority) {
        let trigger = self.current_time + delay;
        if !self.scheduled.contains(&(pos, block.clone())) {
            self.scheduled.insert((pos, block.clone()));
            let sub_tick = self.next_sub_tick();
            self.queue.push(std::cmp::Reverse(ScheduledTick {
                pos, block, trigger_time: trigger, priority, sub_tick,
            }));
        }
    }

    pub fn tick(&mut self, game_time: i64, mut callback: impl FnMut(ScheduledTick<T>)) {
        self.current_time = game_time;
        while let Some(std::cmp::Reverse(tick)) = self.queue.peek() {
            if tick.trigger_time > game_time { break; }
            let std::cmp::Reverse(tick) = self.queue.pop().unwrap();
            self.scheduled.remove(&(tick.pos, tick.block.clone()));
            callback(tick);
        }
    }
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- GameRules ---

    #[test]
    fn game_rules_default_values_match_vanilla() {
        let rules = GameRules::default();
        assert!(rules.get_bool(GameRuleKey::DoDaylightCycle));
        assert!(!rules.get_bool(GameRuleKey::KeepInventory));
        assert_eq!(rules.get_int(GameRuleKey::RandomTickSpeed), 3);
        assert_eq!(rules.get_int(GameRuleKey::SpawnRadius), 8);
        assert_eq!(rules.get_int(GameRuleKey::MaxEntityCramming), 24);
    }

    #[test]
    fn game_rules_set_bool_updates_value() {
        let mut rules = GameRules::default();
        rules.set_bool(GameRuleKey::KeepInventory, true);
        assert!(rules.get_bool(GameRuleKey::KeepInventory));
    }

    #[test]
    fn game_rules_name_roundtrips_all_keys() {
        let keys = [
            GameRuleKey::DoDaylightCycle,
            GameRuleKey::RandomTickSpeed,
            GameRuleKey::KeepInventory,
            GameRuleKey::MobGriefing,
        ];
        for key in keys {
            let name = GameRules::name_of(key);
            assert!(!name.is_empty(), "key {key:?} must have a non-empty name");
        }
    }

    // --- LevelTicks scheduler ---

    #[test]
    fn level_ticks_fires_at_correct_time() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(0, 64, 0);
        ticks.schedule(pos, 42u32, 5, TickPriority::Normal);

        let mut fired = vec![];
        // time 4: not yet
        ticks.tick(4, |t| fired.push(t.block));
        assert!(fired.is_empty(), "should not fire at time 4");

        // time 5: fires
        ticks.tick(5, |t| fired.push(t.block));
        assert_eq!(fired, vec![42]);
    }

    #[test]
    fn level_ticks_deduplicates_same_pos_and_block() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(1, 64, 1);
        ticks.schedule(pos, 1u32, 5, TickPriority::Normal);
        ticks.schedule(pos, 1u32, 5, TickPriority::Normal); // duplicate

        let mut count = 0;
        ticks.tick(10, |_| count += 1);
        assert_eq!(count, 1, "duplicate tick should be deduplicated");
    }

    #[test]
    fn level_ticks_respects_priority_ordering() {
        let mut ticks: LevelTicks<u32> = LevelTicks::new();
        let pos = BlockPos::new(0, 0, 0);
        ticks.schedule(pos, 1u32, 1, TickPriority::Low);
        ticks.schedule(BlockPos::new(1, 0, 0), 2u32, 1, TickPriority::High);

        let mut order = vec![];
        ticks.tick(1, |t| order.push(t.block));
        assert_eq!(order[0], 2, "High priority tick must fire first");
        assert_eq!(order[1], 1, "Low priority tick fires second");
    }

    // --- Day time advancement ---

    #[test]
    fn day_time_wraps_at_24000() {
        let mut level_data = LevelData::default();
        level_data.day_time = 23999;
        // Simulate one tick
        level_data.day_time = (level_data.day_time + 1) % 24000;
        assert_eq!(level_data.day_time, 0);
    }

    #[test]
    fn game_time_never_wraps() {
        let mut level_data = LevelData::default();
        level_data.game_time = i64::MAX - 1;
        level_data.game_time += 1;
        assert_eq!(level_data.game_time, i64::MAX);
    }

    // --- ServerTickRateManager ---

    #[test]
    fn tick_rate_manager_default_is_20_tps() {
        let mgr = ServerTickRateManager::default();
        assert!((mgr.tick_rate - 20.0).abs() < f32::EPSILON);
        assert!(!mgr.frozen);
    }

    #[test]
    fn tick_rate_manager_freeze_and_step() {
        let mut mgr = ServerTickRateManager::default();
        mgr.frozen = true;
        assert!(mgr.is_frozen());
        assert!(!mgr.should_step());

        mgr.request_steps(3);
        assert!(mgr.should_step());
        mgr.consume_step();
        mgr.consume_step();
        mgr.consume_step();
        assert!(!mgr.should_step());
        assert!(mgr.is_frozen(), "still frozen after all steps consumed");
    }

    #[test]
    fn tick_rate_manager_interval_matches_rate() {
        let mgr = ServerTickRateManager { tick_rate: 10.0, ..Default::default() };
        let interval = mgr.tick_interval();
        assert!((interval.as_secs_f32() - 0.1).abs() < 1e-5);
    }
}
```
