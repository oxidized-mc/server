//! Server tick loop — 20 TPS game-state advancement.
//!
//! Runs as a Tokio task, advancing game time, day/night cycle, and weather
//! each tick. Broadcasts [`ClientboundSetTimePacket`] every 20 ticks so
//! clients stay synchronised.
//!
//! The loop respects freeze/step/sprint state from [`ServerTickRateManager`].
//!
//! Corresponds to `net.minecraft.server.MinecraftServer.tickServer()`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use oxidized_game::level::game_rules::GameRuleKey;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::packets::play::{
    ClientboundGameEventPacket, ClientboundSetTimePacket, ClientboundTickingStepPacket,
    GameEventType,
};
use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;
use tokio::sync::broadcast;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, warn};

use crate::network::{BroadcastMessage, ServerContext};

/// Overload warning threshold — if a single tick exceeds this, log a warning.
const OVERLOAD_THRESHOLD: Duration = Duration::from_millis(2000);

/// Delay before rain starts (12 000–180 000 ticks = 10 min–2.5 hours).
const RAIN_DELAY_MIN: i32 = 12_000;
const RAIN_DELAY_MAX: i32 = 180_000;

/// Duration rain lasts once started (12 000–24 000 ticks = 10–20 min).
const RAIN_DURATION_MIN: i32 = 12_000;
const RAIN_DURATION_MAX: i32 = 24_000;

/// Delay before thunder starts (12 000–180 000 ticks = 10 min–2.5 hours).
const THUNDER_DELAY_MIN: i32 = 12_000;
const THUNDER_DELAY_MAX: i32 = 180_000;

/// Duration thunder lasts once started (3 600–15 600 ticks = 3–13 min).
const THUNDER_DURATION_MIN: i32 = 3_600;
const THUNDER_DURATION_MAX: i32 = 15_600;

/// Per-tick delta for rain/thunder visual level interpolation.
const WEATHER_LEVEL_DELTA: f32 = 0.01;

/// Tracks interpolated rain/thunder visual intensity levels across ticks.
struct WeatherLevels {
    rain_level: f32,
    old_rain_level: f32,
    thunder_level: f32,
    old_thunder_level: f32,
}

/// Runs the main server tick loop until `shutdown_rx` fires.
///
/// Should be spawned as a Tokio task:
/// ```ignore
/// tokio::spawn(tick::run_tick_loop(ctx.clone(), shutdown_rx));
/// ```
pub async fn run_tick_loop(ctx: Arc<ServerContext>, mut shutdown_rx: broadcast::Receiver<()>) {
    let mut tick_count: u64 = 0;
    let mut timer = interval(Duration::from_millis(50));
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut rng = rand::rngs::SmallRng::seed_from_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    let mut weather = WeatherLevels {
        rain_level: 0.0,
        old_rain_level: 0.0,
        thunder_level: 0.0,
        old_thunder_level: 0.0,
    };

    debug!("Tick loop started at 20 TPS");

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                debug!("Tick loop shutting down");
                break;
            }

            _ = timer.tick() => {
                let tick_start = Instant::now();

                // Snapshot tick rate manager state in a single lock acquisition.
                let (should_tick, new_interval, frozen, steps_remaining) = {
                    let mut mgr = ctx.tick_rate_manager.write();
                    let should = mgr.should_tick();
                    let interval = mgr.tick_interval();
                    (should, interval, mgr.frozen, mgr.steps_remaining)
                };

                if should_tick {
                    do_tick(&ctx, tick_count, &mut rng, &mut weather).await;
                    tick_count += 1;
                }

                // Update interval based on snapshotted tick rate.
                timer.reset_after(new_interval.saturating_sub(tick_start.elapsed()));

                // Overload detection.
                let elapsed = tick_start.elapsed();
                if elapsed > OVERLOAD_THRESHOLD {
                    warn!(
                        elapsed_ms = elapsed.as_millis(),
                        "Can't keep up! Tick took {}ms (threshold: {}ms)",
                        elapsed.as_millis(),
                        OVERLOAD_THRESHOLD.as_millis(),
                    );
                }

                // Broadcast stepping state to clients when frozen.
                if frozen {
                    broadcast_packet(
                        &ctx,
                        &ClientboundTickingStepPacket {
                            tick_steps: steps_remaining as i32,
                        },
                    );
                }
            }
        }
    }

    debug!("Tick loop stopped after {tick_count} ticks");
}

/// Performs one game tick.
async fn do_tick(
    ctx: &ServerContext,
    tick_count: u64,
    rng: &mut impl Rng,
    weather: &mut WeatherLevels,
) {
    // Snapshot game rules once per tick to minimize lock acquisitions.
    let (do_daylight, do_weather) = {
        let rules = ctx.game_rules.read();
        (
            rules.get_bool(GameRuleKey::AdvanceTime),
            rules.get_bool(GameRuleKey::AdvanceWeather),
        )
    };

    // Advance world time.
    {
        let mut ld = ctx.level_data.write();
        ld.time = ld.time.wrapping_add(1);

        // Advance day time if the advance_time game rule is enabled.
        // Day time grows unbounded — the client uses `day_time % 24000` for rendering.
        if do_daylight {
            ld.day_time = ld.day_time.wrapping_add(1);
        }
    }

    // Tick weather (includes state transitions + client broadcasts).
    if do_weather {
        tick_weather(ctx, rng, weather);
    }

    // Broadcast time to all clients every 20 ticks (once per second).
    if tick_count % 20 == 0 {
        broadcast_time(ctx, do_daylight);
    }

    // Autosave level.dat at a dynamic interval: max(100, tps * 300) ticks.
    let autosave_interval = {
        let mgr = ctx.tick_rate_manager.read();
        (mgr.tick_rate * 300.0).max(100.0) as u64
    };
    if tick_count > 0 && tick_count % autosave_interval == 0 {
        autosave_level_dat(ctx).await;
    }
}

/// Advances weather countdown timers, transitions weather states, interpolates
/// visual levels, and broadcasts changes to clients.
///
/// Mirrors `ServerLevel.advanceWeatherCycle()` from vanilla 26.1.
fn tick_weather(ctx: &ServerContext, rng: &mut impl Rng, weather: &mut WeatherLevels) {
    let was_raining;
    {
        let mut ld = ctx.level_data.write();
        was_raining = ld.is_raining;

        let mut clear_weather_time = ld.clear_weather_time;
        let mut thunder_time = ld.thunder_time;
        let mut rain_time = ld.rain_time;
        let mut thundering = ld.is_thundering;
        let mut raining = ld.is_raining;

        if clear_weather_time > 0 {
            // `/weather clear <duration>` forces clear weather.
            clear_weather_time -= 1;
            thunder_time = if thundering { 0 } else { 1 };
            rain_time = if raining { 0 } else { 1 };
            thundering = false;
            raining = false;
        } else {
            // Thunder state machine.
            if thunder_time > 0 {
                thunder_time -= 1;
                if thunder_time == 0 {
                    thundering = !thundering;
                }
            } else if thundering {
                thunder_time = rng.random_range(THUNDER_DURATION_MIN..=THUNDER_DURATION_MAX);
            } else {
                thunder_time = rng.random_range(THUNDER_DELAY_MIN..=THUNDER_DELAY_MAX);
            }

            // Rain state machine.
            if rain_time > 0 {
                rain_time -= 1;
                if rain_time == 0 {
                    raining = !raining;
                }
            } else if raining {
                rain_time = rng.random_range(RAIN_DURATION_MIN..=RAIN_DURATION_MAX);
            } else {
                rain_time = rng.random_range(RAIN_DELAY_MIN..=RAIN_DELAY_MAX);
            }
        }

        ld.clear_weather_time = clear_weather_time;
        ld.thunder_time = thunder_time;
        ld.rain_time = rain_time;
        ld.is_thundering = thundering;
        ld.is_raining = raining;
    }

    // Interpolate visual levels (±0.01 per tick, clamped to 0.0–1.0).
    let (raining, thundering) = {
        let ld = ctx.level_data.read();
        (ld.is_raining, ld.is_thundering)
    };

    weather.old_thunder_level = weather.thunder_level;
    if thundering {
        weather.thunder_level = (weather.thunder_level + WEATHER_LEVEL_DELTA).min(1.0);
    } else {
        weather.thunder_level = (weather.thunder_level - WEATHER_LEVEL_DELTA).max(0.0);
    }

    weather.old_rain_level = weather.rain_level;
    if raining {
        weather.rain_level = (weather.rain_level + WEATHER_LEVEL_DELTA).min(1.0);
    } else {
        weather.rain_level = (weather.rain_level - WEATHER_LEVEL_DELTA).max(0.0);
    }

    // Broadcast gradual level changes.
    if weather.old_rain_level != weather.rain_level {
        broadcast_packet(
            ctx,
            &ClientboundGameEventPacket {
                event: GameEventType::RainLevelChange,
                param: weather.rain_level,
            },
        );
    }
    if weather.old_thunder_level != weather.thunder_level {
        broadcast_packet(
            ctx,
            &ClientboundGameEventPacket {
                event: GameEventType::ThunderLevelChange,
                param: weather.thunder_level,
            },
        );
    }

    // Broadcast start/stop transitions.
    if was_raining != raining {
        let event = if was_raining {
            GameEventType::StopRaining
        } else {
            GameEventType::StartRaining
        };
        broadcast_packet(ctx, &ClientboundGameEventPacket { event, param: 0.0 });
        // Also send current levels so the client syncs immediately.
        broadcast_packet(
            ctx,
            &ClientboundGameEventPacket {
                event: GameEventType::RainLevelChange,
                param: weather.rain_level,
            },
        );
        broadcast_packet(
            ctx,
            &ClientboundGameEventPacket {
                event: GameEventType::ThunderLevelChange,
                param: weather.thunder_level,
            },
        );
    }
}

/// Broadcasts a [`ClientboundSetTimePacket`] to all connected players.
///
/// Periodic syncs send empty `clock_updates` (vanilla behaviour). Full
/// clock data is only sent on join or when the clock parameters change.
fn broadcast_time(ctx: &ServerContext, _do_daylight: bool) {
    let ld = ctx.level_data.read();
    let pkt = ClientboundSetTimePacket {
        game_time: ld.time,
        clock_updates: vec![],
    };
    drop(ld);
    broadcast_packet(ctx, &pkt);
}

/// Encodes and broadcasts any packet through the broadcast channel.
fn broadcast_packet<P: Packet>(ctx: &ServerContext, pkt: &P) {
    let encoded = pkt.encode();
    let msg = BroadcastMessage {
        packet_id: P::PACKET_ID,
        data: encoded.freeze(),
        exclude_entity: None,
        target_entity: None,
    };
    let _ = ctx.broadcast_tx.send(msg);
}

/// Saves level.dat to disk via `spawn_blocking` (ADR-015).
///
/// Takes a snapshot of `level_data` under a read lock, then writes to disk
/// on a blocking thread. Returns `Ok(())` on success or the error.
///
/// Used by both the periodic autosave and the shutdown save path.
pub(crate) async fn save_level_dat(
    ctx: &ServerContext,
) -> Result<(), Box<dyn std::error::Error + Send>> {
    let level_data = ctx.level_data.read().clone();
    let level_dat_path = ctx.storage.level_dat_path();
    let result = tokio::task::spawn_blocking(move || level_data.save(&level_dat_path)).await;
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(Box::new(e)),
        Err(e) => Err(Box::new(e)),
    }
}

/// Autosaves level.dat, logging the outcome without crashing the server.
async fn autosave_level_dat(ctx: &ServerContext) {
    debug!("Autosaving level.dat...");
    match save_level_dat(ctx).await {
        Ok(()) => debug!("Autosave complete"),
        Err(e) => warn!(error = %e, "Autosave failed for level.dat"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_game::level::{GameRules, ServerTickRateManager};
    use oxidized_game::player::PlayerList;
    use oxidized_protocol::types::resource_location::ResourceLocation;
    use oxidized_world::storage::{LevelStorageSource, PrimaryLevelData};
    use parking_lot::RwLock;
    use rand::SeedableRng;

    const TICKS_PER_DAY: i64 = 24_000;

    fn test_ctx() -> Arc<ServerContext> {
        use oxidized_world::registry::BlockRegistry;

        Arc::new(ServerContext {
            player_list: RwLock::new(PlayerList::new(20)),
            level_data: RwLock::new(
                PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
            ),
            dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
            max_view_distance: 10,
            max_simulation_distance: 10,
            broadcast_tx: broadcast::channel(256).0,
            color_char: None,
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            max_players: 20,
            shutdown_tx: broadcast::channel(1).0,
            game_rules: RwLock::new(GameRules::default()),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
            storage: LevelStorageSource::new(""),
            chunks: dashmap::DashMap::new(),
            dirty_chunks: dashmap::DashSet::new(),
            block_registry: Arc::new(BlockRegistry::load().unwrap()),
            chunk_generator: Arc::new(oxidized_game::worldgen::flat::FlatChunkGenerator::new(
                oxidized_game::worldgen::flat::FlatWorldConfig::default(),
            )),
        })
    }

    fn test_rng() -> rand::rngs::SmallRng {
        rand::rngs::SmallRng::seed_from_u64(42)
    }

    fn test_weather() -> WeatherLevels {
        WeatherLevels {
            rain_level: 0.0,
            old_rain_level: 0.0,
            thunder_level: 0.0,
            old_thunder_level: 0.0,
        }
    }

    #[tokio::test]
    async fn test_do_tick_advances_time() {
        let ctx = test_ctx();
        let initial_time = ctx.level_data.read().time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather()).await;
        assert_eq!(ctx.level_data.read().time, initial_time + 1);
    }

    #[tokio::test]
    async fn test_do_tick_advances_day_time() {
        let ctx = test_ctx();
        let initial_day = ctx.level_data.read().day_time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather()).await;
        assert_eq!(ctx.level_data.read().day_time, initial_day + 1);
    }

    #[tokio::test]
    async fn test_day_time_grows_unbounded() {
        let ctx = test_ctx();
        ctx.level_data.write().day_time = TICKS_PER_DAY - 1;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather()).await;
        assert_eq!(
            ctx.level_data.read().day_time,
            TICKS_PER_DAY,
            "day_time should grow past 24000 (client handles modulo)"
        );
    }

    #[tokio::test]
    async fn test_daylight_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.game_rules
            .write()
            .set_bool(GameRuleKey::AdvanceTime, false);
        let initial_day = ctx.level_data.read().day_time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather()).await;
        assert_eq!(
            ctx.level_data.read().day_time,
            initial_day,
            "day_time should not advance when advance_time is false"
        );
        // But game_time always advances.
        assert_eq!(ctx.level_data.read().time, 1);
    }

    #[tokio::test]
    async fn test_weather_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.game_rules
            .write()
            .set_bool(GameRuleKey::AdvanceWeather, false);
        ctx.level_data.write().rain_time = 1; // would flip without gamerule
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather()).await;
        // Weather should not have changed because gamerule is false.
        assert_eq!(
            ctx.level_data.read().rain_time,
            1,
            "weather should not tick when advance_weather is false"
        );
    }

    #[test]
    fn test_weather_transitions() {
        let ctx = test_ctx();
        ctx.level_data.write().is_raining = false;
        ctx.level_data.write().rain_time = 1;
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        assert!(ctx.level_data.read().is_raining, "should start raining");
    }

    #[test]
    fn test_thunder_independent_of_rain() {
        // In vanilla 26.1, thunder and rain are independent state machines.
        // Thunder doesn't require rain — both can toggle independently.
        let ctx = test_ctx();
        {
            let mut ld = ctx.level_data.write();
            ld.is_raining = false;
            ld.is_thundering = true;
            ld.thunder_time = 5;
        }
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        // Thunder remains true because its timer hasn't reached 0.
        assert!(
            ctx.level_data.read().is_thundering,
            "thunder should persist independently of rain"
        );
    }

    #[test]
    fn test_clear_weather_time_overrides() {
        let ctx = test_ctx();
        {
            let mut ld = ctx.level_data.write();
            ld.is_raining = true;
            ld.is_thundering = true;
            ld.clear_weather_time = 100;
            ld.rain_time = 5000;
            ld.thunder_time = 3000;
        }
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        let ld = ctx.level_data.read();
        assert!(!ld.is_raining, "clear weather should stop rain");
        assert!(!ld.is_thundering, "clear weather should stop thunder");
        assert_eq!(
            ld.clear_weather_time, 99,
            "clear weather timer should decrement"
        );
    }

    #[test]
    fn test_weather_random_duration_in_range() {
        let ctx = test_ctx();
        // Set rain_time to 0 while raining to trigger duration sample.
        {
            let mut ld = ctx.level_data.write();
            ld.is_raining = true;
            ld.rain_time = 0;
        }
        tick_weather(
            &ctx,
            &mut rand::rngs::SmallRng::seed_from_u64(12345),
            &mut test_weather(),
        );
        let rain_time = ctx.level_data.read().rain_time;
        assert!(
            (RAIN_DURATION_MIN..=RAIN_DURATION_MAX).contains(&rain_time),
            "rain duration {rain_time} should be in range {RAIN_DURATION_MIN}..={RAIN_DURATION_MAX}"
        );
    }

    #[tokio::test]
    async fn test_tick_loop_shutdown() {
        let ctx = test_ctx();
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let ctx_clone = ctx.clone();
        let handle = tokio::spawn(async move {
            run_tick_loop(ctx_clone, shutdown_rx).await;
        });

        // Let a few ticks run.
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "tick loop should shut down promptly");
        assert!(
            ctx.level_data.read().time > 0,
            "should have ticked at least once"
        );
    }
}
