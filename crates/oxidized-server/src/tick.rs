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
    ClientboundSetTimePacket, ClientboundTickingStatePacket, ClientboundTickingStepPacket,
    ClockNetworkState, ClockUpdate,
};
use tokio::sync::broadcast;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, warn};

use crate::network::{ChatBroadcastMessage, ServerContext};

/// Ticks in one Minecraft day (24 000).
const TICKS_PER_DAY: i64 = 24_000;

/// Overload warning threshold — if a single tick exceeds this, log a warning.
const OVERLOAD_THRESHOLD: Duration = Duration::from_millis(2000);

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

                // Check tick rate manager for freeze/step/sprint state.
                let should_tick = ctx.tick_rate_manager.write().should_tick();

                if should_tick {
                    do_tick(&ctx, tick_count);
                    tick_count += 1;
                }

                // Update interval if tick rate changed.
                let new_interval = ctx.tick_rate_manager.read().tick_interval();
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
                if ctx.tick_rate_manager.read().frozen {
                    let steps = ctx.tick_rate_manager.read().steps_remaining;
                    broadcast_packet(
                        &ctx,
                        &ClientboundTickingStepPacket {
                            tick_steps: steps as i32,
                        },
                    );
                }
            }
        }
    }

    debug!("Tick loop stopped after {tick_count} ticks");
}

/// Performs one game tick.
fn do_tick(ctx: &ServerContext, tick_count: u64) {
    // Advance world time.
    {
        let mut ld = ctx.level_data.write();
        ld.time = ld.time.wrapping_add(1);

        // Advance day time if the doDaylightCycle game rule is enabled.
        let do_daylight = ctx.game_rules.read().get_bool(GameRuleKey::DoDaylightCycle);
        if do_daylight {
            ld.day_time = (ld.day_time + 1) % TICKS_PER_DAY;
        }
    }

    // Tick weather.
    {
        let do_weather = ctx.game_rules.read().get_bool(GameRuleKey::DoWeatherCycle);
        if do_weather {
            tick_weather(ctx);
        }
    }

    // Broadcast time to all clients every 20 ticks (once per second).
    if tick_count % 20 == 0 {
        broadcast_time(ctx);
    }
}

/// Advances weather countdown timers and transitions weather states.
fn tick_weather(ctx: &ServerContext) {
    let mut ld = ctx.level_data.write();

    // Thunder timer.
    if ld.thunder_time > 0 {
        ld.thunder_time -= 1;
        if ld.thunder_time == 0 {
            ld.is_thundering = !ld.is_thundering;
            // Reset to random duration (6000–18000 ticks).
            ld.thunder_time = if ld.is_thundering {
                // Thunder lasts 3600–15600 ticks.
                3600
            } else {
                // Clear period between thunderstorms.
                12000
            };
        }
    }

    // Rain timer.
    if ld.rain_time > 0 {
        ld.rain_time -= 1;
        if ld.rain_time == 0 {
            ld.is_raining = !ld.is_raining;
            ld.rain_time = if ld.is_raining {
                // Rain lasts 12000–24000 ticks.
                12000
            } else {
                // Dry period.
                12000
            };
        }
    }

    // Thunder requires rain.
    if ld.is_thundering && !ld.is_raining {
        ld.is_thundering = false;
    }
}

/// Broadcasts a [`ClientboundSetTimePacket`] to all connected players.
fn broadcast_time(ctx: &ServerContext) {
    let ld = ctx.level_data.read();
    let pkt = ClientboundSetTimePacket {
        game_time: ld.time,
        clock_updates: vec![ClockUpdate {
            clock_id: ClientboundSetTimePacket::OVERWORLD_CLOCK_ID,
            state: ClockNetworkState {
                total_ticks: ld.day_time,
                partial_tick: 0.0,
                rate: if ctx.game_rules.read().get_bool(GameRuleKey::DoDaylightCycle) {
                    1.0
                } else {
                    0.0
                },
            },
        }],
    };
    drop(ld);
    broadcast_packet(ctx, &pkt);
}

/// Encodes and broadcasts any packet through the chat broadcast channel.
fn broadcast_packet<P: Packet>(ctx: &ServerContext, pkt: &P) {
    let encoded = pkt.encode();
    let msg = ChatBroadcastMessage {
        packet_id: P::PACKET_ID,
        data: encoded.freeze(),
    };
    let _ = ctx.chat_tx.send(msg);
}

/// Broadcasts tick rate state to all clients.
#[allow(dead_code)]
pub fn broadcast_tick_state(ctx: &ServerContext) {
    let mgr = ctx.tick_rate_manager.read();
    let pkt = ClientboundTickingStatePacket {
        tick_rate: mgr.tick_rate,
        is_frozen: mgr.frozen,
    };
    drop(mgr);
    broadcast_packet(ctx, &pkt);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use oxidized_game::level::{GameRules, ServerTickRateManager};
    use oxidized_game::player::PlayerList;
    use oxidized_protocol::types::resource_location::ResourceLocation;
    use oxidized_world::storage::PrimaryLevelData;
    use parking_lot::RwLock;

    fn test_ctx() -> Arc<ServerContext> {
        Arc::new(ServerContext {
            player_list: RwLock::new(PlayerList::new(20)),
            level_data: RwLock::new(
                PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
            ),
            dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
            max_view_distance: 10,
            max_simulation_distance: 10,
            chat_tx: broadcast::channel(256).0,
            color_char: None,
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            max_players: 20,
            shutdown_tx: broadcast::channel(1).0,
            game_rules: RwLock::new(GameRules::default()),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
        })
    }

    #[test]
    fn test_do_tick_advances_time() {
        let ctx = test_ctx();
        let initial_time = ctx.level_data.read().time;
        do_tick(&ctx, 0);
        assert_eq!(ctx.level_data.read().time, initial_time + 1);
    }

    #[test]
    fn test_do_tick_advances_day_time() {
        let ctx = test_ctx();
        let initial_day = ctx.level_data.read().day_time;
        do_tick(&ctx, 0);
        assert_eq!(ctx.level_data.read().day_time, initial_day + 1);
    }

    #[test]
    fn test_day_time_wraps_at_24000() {
        let ctx = test_ctx();
        ctx.level_data.write().day_time = TICKS_PER_DAY - 1;
        do_tick(&ctx, 0);
        assert_eq!(ctx.level_data.read().day_time, 0);
    }

    #[test]
    fn test_daylight_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.game_rules
            .write()
            .set_bool(GameRuleKey::DoDaylightCycle, false);
        let initial_day = ctx.level_data.read().day_time;
        do_tick(&ctx, 0);
        assert_eq!(
            ctx.level_data.read().day_time,
            initial_day,
            "day_time should not advance when doDaylightCycle is false"
        );
        // But game_time always advances.
        assert_eq!(ctx.level_data.read().time, 1);
    }

    #[test]
    fn test_weather_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.game_rules
            .write()
            .set_bool(GameRuleKey::DoWeatherCycle, false);
        ctx.level_data.write().rain_time = 1; // would flip without gamerule
        do_tick(&ctx, 0);
        // Weather should not have changed because gamerule is false.
        assert_eq!(
            ctx.level_data.read().rain_time,
            1,
            "weather should not tick when doWeatherCycle is false"
        );
    }

    #[test]
    fn test_weather_transitions() {
        let ctx = test_ctx();
        ctx.level_data.write().is_raining = false;
        ctx.level_data.write().rain_time = 1;
        tick_weather(&ctx);
        assert!(ctx.level_data.read().is_raining, "should start raining");
    }

    #[test]
    fn test_thunder_requires_rain() {
        let ctx = test_ctx();
        {
            let mut ld = ctx.level_data.write();
            ld.is_raining = false;
            ld.is_thundering = true;
        }
        tick_weather(&ctx);
        assert!(
            !ctx.level_data.read().is_thundering,
            "thunder cleared without rain"
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
