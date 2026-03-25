//! Server tick loop — 20 TPS game-state advancement.
//!
//! Runs on a **dedicated OS thread** (ADR-019), advancing game time,
//! day/night cycle, and weather each tick. Broadcasts
//! [`ClientboundSetTimePacket`] every 20 ticks so clients stay synchronised.
//!
//! The loop respects freeze/step/sprint state from [`ServerTickRateManager`](oxidized_game::level::ServerTickRateManager).
//!
//! Corresponds to `net.minecraft.server.MinecraftServer.tickServer()`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use oxidized_game::level::game_rules::GameRuleKey;
use oxidized_game::lighting::engine::LightEngine;
use oxidized_game::net::light_serializer::build_light_data;
use oxidized_protocol::codec::Packet;
use oxidized_protocol::constants::TICKS_PER_SECOND;
use oxidized_protocol::packets::play::{
    ClientboundGameEventPacket, ClientboundLightUpdatePacket, ClientboundSetTimePacket,
    ClientboundTickingStepPacket, ClockNetworkState, ClockUpdate, GameEventType,
};
use oxidized_world::anvil::{RegionFile, compress_zlib};
use oxidized_world::chunk::ChunkPos;
use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;
use tracing::{debug, error, info, warn};

use crate::network::{BroadcastMessage, ServerContext};

/// Warning threshold — if a single tick exceeds this, log a warning.
const OVERLOAD_WARNING_THRESHOLD: Duration = Duration::from_millis(100);

/// Critical threshold — if a single tick exceeds this, log an error.
const OVERLOAD_CRITICAL_THRESHOLD: Duration = Duration::from_millis(500);

/// Per-tick delta for rain/thunder visual level interpolation.
const WEATHER_LEVEL_DELTA: f32 = 0.01;

/// Broadcast time to clients every this many ticks (once per real-time second).
const TIME_BROADCAST_INTERVAL: u64 = TICKS_PER_SECOND as u64;

/// Autosave interval scale: `tick_rate × AUTOSAVE_RATE_MULTIPLIER` ticks (~5 min).
const AUTOSAVE_RATE_MULTIPLIER: f32 = 300.0;

/// Floor for the dynamic autosave interval in ticks.
const AUTOSAVE_MIN_INTERVAL: f32 = 100.0;

/// Tracks interpolated rain/thunder visual intensity levels across ticks.
struct WeatherLevels {
    rain_level: f32,
    old_rain_level: f32,
    thunder_level: f32,
    old_thunder_level: f32,
}

/// Runs the main server tick loop until `shutdown` is set to `true`.
///
/// Must be called on a **dedicated OS thread** (ADR-019):
/// ```ignore
/// let shutdown = Arc::new(AtomicBool::new(false));
/// std::thread::Builder::new()
///     .name("tick".into())
///     .spawn({
///         let shutdown = shutdown.clone();
///         move || tick::run_tick_loop(&ctx, ecs, &shutdown)
///     })?;
/// ```
pub fn run_tick_loop(
    ctx: &ServerContext,
    mut ecs: crate::ecs::EcsContext,
    shutdown: &AtomicBool,
) {
    let mut tick_count: u64 = 0;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_else(|e| {
            warn!("System clock error, using fallback RNG seed: {e}");
            rand::random::<u64>()
        });
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut weather = WeatherLevels {
        rain_level: 0.0,
        old_rain_level: 0.0,
        thunder_level: 0.0,
        old_thunder_level: 0.0,
    };

    info!(
        thread = ?std::thread::current().name(),
        "Tick loop started at 20 TPS on dedicated thread",
    );

    loop {
        if shutdown.load(Ordering::Relaxed) {
            debug!("Tick loop shutting down");
            break;
        }

        let tick_start = Instant::now();

        // Snapshot tick rate manager state in a single lock acquisition.
        let (should_tick, target_interval, is_frozen, steps_remaining) = {
            let mut mgr = ctx.tick_rate_manager.write();
            let should = mgr.should_tick();
            let interval = mgr.tick_interval();
            (should, interval, mgr.is_frozen, mgr.steps_remaining)
        };

        if should_tick {
            do_tick(ctx, tick_count, &mut rng, &mut weather);

            // Run all 7 ECS phases sequentially (ADR-018, ADR-019).
            ecs.run_tick();

            // Drain outbound entity packets and broadcast them.
            drain_outbound_entity_packets(ctx, &mut ecs);

            tick_count += 1;
        }

        // Overload detection (two-tier).
        let elapsed = tick_start.elapsed();
        if elapsed > OVERLOAD_CRITICAL_THRESHOLD {
            error!(
                elapsed_ms = elapsed.as_millis(),
                "Can't keep up! Tick took {}ms (critical threshold: {}ms)",
                elapsed.as_millis(),
                OVERLOAD_CRITICAL_THRESHOLD.as_millis(),
            );
        } else if elapsed > OVERLOAD_WARNING_THRESHOLD {
            warn!(
                elapsed_ms = elapsed.as_millis(),
                "Tick running behind! Took {}ms (warning threshold: {}ms)",
                elapsed.as_millis(),
                OVERLOAD_WARNING_THRESHOLD.as_millis(),
            );
        }

        // Broadcast stepping state to clients when frozen.
        if is_frozen {
            broadcast_packet(
                ctx,
                &ClientboundTickingStepPacket {
                    tick_steps: steps_remaining as i32,
                },
            );
        }

        // Sleep for the remaining tick budget (skip-style: if behind, don't sleep).
        let remaining = target_interval.saturating_sub(tick_start.elapsed());
        if !remaining.is_zero() {
            std::thread::sleep(remaining);
        }
    }

    debug!("Tick loop stopped after {tick_count} ticks");
}

/// Performs one game tick.
fn do_tick(ctx: &ServerContext, tick_count: u64, rng: &mut impl Rng, weather: &mut WeatherLevels) {
    // Snapshot game rules once per tick to minimize lock acquisitions.
    let (do_daylight, do_weather) = {
        let rules = ctx.world.game_rules.read();
        (
            rules.get_bool(GameRuleKey::AdvanceTime),
            rules.get_bool(GameRuleKey::AdvanceWeather),
        )
    };

    // Advance world time.
    {
        let mut ld = ctx.world.level_data.write();
        ld.time.game_time = ld.time.game_time.wrapping_add(1);

        // Advance day time if the advance_time game rule is enabled.
        // Day time grows unbounded — the client uses `day_time % 24000` for rendering.
        if do_daylight {
            ld.time.day_time = ld.time.day_time.wrapping_add(1);
        }
    }

    // Tick weather (includes state transitions + client broadcasts).
    if do_weather {
        tick_weather(ctx, rng, weather);
    }

    // Broadcast time to all clients every TIME_BROADCAST_INTERVAL ticks.
    if tick_count % TIME_BROADCAST_INTERVAL == 0 {
        broadcast_time(ctx, do_daylight);
    }

    // Autosave level.dat at a dynamic interval: max(100, tps * 300) ticks.
    let autosave_interval = {
        let mgr = ctx.tick_rate_manager.read();
        (mgr.tick_rate * AUTOSAVE_RATE_MULTIPLIER).max(AUTOSAVE_MIN_INTERVAL) as u64
    };
    if tick_count > 0 && tick_count % autosave_interval == 0 {
        autosave_level_dat(ctx);
        autosave_chunks(ctx);
    }

    // Process any pending light updates from block changes this tick.
    process_light_updates(ctx);
}

/// Drains pending light updates, processes them per-chunk, and broadcasts
/// light update packets to all connected clients.
fn process_light_updates(ctx: &ServerContext) {
    let updates = {
        let mut queue = ctx.world.pending_light_updates.lock();
        if queue.is_empty() {
            return;
        }
        std::mem::take(&mut *queue)
    };

    // Group updates by chunk position.
    let mut by_chunk: HashMap<ChunkPos, Vec<oxidized_game::lighting::queue::LightUpdate>> =
        HashMap::new();
    for (chunk_pos, update) in updates {
        by_chunk.entry(chunk_pos).or_default().push(update);
    }

    // Process each chunk's updates and broadcast light packets.
    for (chunk_pos, chunk_updates) in &by_chunk {
        let Some(chunk_ref) = ctx.world.chunks.get(chunk_pos) else {
            continue;
        };

        let mut chunk = chunk_ref.write();
        let mut engine = LightEngine::new();
        for update in chunk_updates {
            engine.queue_mut().push(update.clone());
        }

        match engine.process_updates(&mut chunk) {
            Ok(changed_sections) if !changed_sections.is_empty() => {
                let light_data =
                    build_light_data(chunk.sky_light_layers(), chunk.block_light_layers());
                drop(chunk); // Release write lock before broadcasting.
                let pkt = ClientboundLightUpdatePacket {
                    chunk_x: chunk_pos.x,
                    chunk_z: chunk_pos.z,
                    light_data,
                };
                let data = pkt.encode();
                ctx.broadcast(BroadcastMessage {
                    packet_id: ClientboundLightUpdatePacket::PACKET_ID,
                    data: data.freeze(),
                    exclude_entity: None,
                    target_entity: None,
                });
            },
            Err(e) => {
                warn!(chunk = ?chunk_pos, error = %e, "Light update failed");
            },
            _ => {},
        }
    }
}

/// Drains outbound entity packets produced by ECS systems and broadcasts
/// them to all connected players.
fn drain_outbound_entity_packets(ctx: &ServerContext, ecs: &mut crate::ecs::EcsContext) {
    use oxidized_game::entity::systems::OutboundEntityPackets;

    let packets = {
        let mut res = ecs.world.resource_mut::<OutboundEntityPackets>();
        std::mem::take(&mut res.0)
    };

    for pkt in packets {
        ctx.broadcast(BroadcastMessage {
            packet_id: pkt.packet_id,
            data: pkt.data,
            exclude_entity: None,
            target_entity: None,
        });
    }
}

/// Advances weather countdown timers, transitions weather states, interpolates
/// visual levels, and broadcasts changes to clients.
///
/// Mirrors `ServerLevel.advanceWeatherCycle()` from vanilla 26.1.
fn tick_weather(ctx: &ServerContext, rng: &mut impl Rng, weather: &mut WeatherLevels) {
    let was_raining;
    let w = &ctx.settings.weather;
    {
        let mut ld = ctx.world.level_data.write();
        was_raining = ld.weather.is_raining;

        let mut clear_weather_time = ld.weather.clear_weather_time;
        let mut thunder_time = ld.weather.thunder_time;
        let mut rain_time = ld.weather.rain_time;
        let mut thundering = ld.weather.is_thundering;
        let mut raining = ld.weather.is_raining;

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
                thunder_time = rng.random_range(w.thunder_duration_min..=w.thunder_duration_max);
            } else {
                thunder_time = rng.random_range(w.thunder_delay_min..=w.thunder_delay_max);
            }

            // Rain state machine.
            if rain_time > 0 {
                rain_time -= 1;
                if rain_time == 0 {
                    raining = !raining;
                }
            } else if raining {
                rain_time = rng.random_range(w.rain_duration_min..=w.rain_duration_max);
            } else {
                rain_time = rng.random_range(w.rain_delay_min..=w.rain_delay_max);
            }
        }

        ld.weather.clear_weather_time = clear_weather_time;
        ld.weather.thunder_time = thunder_time;
        ld.weather.rain_time = rain_time;
        ld.weather.is_thundering = thundering;
        ld.weather.is_raining = raining;
    }

    // Interpolate visual levels (±0.01 per tick, clamped to 0.0–1.0).
    let (raining, thundering) = {
        let ld = ctx.world.level_data.read();
        (ld.weather.is_raining, ld.weather.is_thundering)
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
/// Includes the overworld clock update so clients synchronise their
/// day/night cycle each broadcast (every 20 ticks).
fn broadcast_time(ctx: &ServerContext, _do_daylight: bool) {
    let ld = ctx.world.level_data.read();
    let pkt = ClientboundSetTimePacket {
        game_time: ld.time.game_time,
        clock_updates: vec![ClockUpdate {
            clock_id: ClientboundSetTimePacket::OVERWORLD_CLOCK_ID,
            state: ClockNetworkState {
                total_ticks: ld.time.day_time,
                partial_tick: 0.0,
                rate: 1.0,
            },
        }],
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
    ctx.broadcast(msg);
}

/// Saves level.dat synchronously with direct file I/O.
///
/// Suitable for the tick thread (which is allowed to block) and other
/// non-async contexts.
fn save_level_dat_blocking(ctx: &ServerContext) -> Result<(), Box<dyn std::error::Error + Send>> {
    let level_data = ctx.world.level_data.read().clone();
    let level_dat_path = ctx.world.storage.level_dat_path();
    level_data
        .save(&level_dat_path)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)
}

/// Saves level.dat asynchronously via `spawn_blocking` (ADR-015).
///
/// Takes a snapshot of `level_data` under a read lock, then writes to disk
/// on a blocking thread. Returns `Ok(())` on success or the error.
///
/// Used by the shutdown save path (which runs in async context).
pub async fn save_level_dat(ctx: &ServerContext) -> Result<(), Box<dyn std::error::Error + Send>> {
    let level_data = ctx.world.level_data.read().clone();
    let level_dat_path = ctx.world.storage.level_dat_path();
    let result = tokio::task::spawn_blocking(move || level_data.save(&level_dat_path)).await;
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(Box::new(e)),
        Err(e) => Err(Box::new(e)),
    }
}

/// Autosaves level.dat from the tick thread, logging the outcome.
fn autosave_level_dat(ctx: &ServerContext) {
    debug!("Autosaving level.dat...");
    match save_level_dat_blocking(ctx) {
        Ok(()) => debug!("Autosave complete"),
        Err(e) => warn!(error = %e, "Autosave failed for level.dat"),
    }
}

/// Saves all dirty chunks to region files synchronously.
///
/// Drains the `dirty_chunks` set, serializes each chunk using
/// [`ChunkSerializer`], compresses with zlib, and writes to the
/// appropriate `.mca` region file. Chunks are grouped by region
/// to minimise file opens/flushes.
///
/// Returns the number of chunks saved.
///
/// # Errors
///
/// Returns the first I/O error encountered. Chunks already written
/// before the error remain on disk.
pub fn save_dirty_chunks_blocking(
    ctx: &ServerContext,
) -> Result<usize, Box<dyn std::error::Error + Send>> {
    let dirty: Vec<ChunkPos> = ctx.world.dirty_chunks.iter().map(|r| *r).collect();
    if dirty.is_empty() {
        return Ok(0);
    }

    let region_dir = ctx
        .world
        .storage
        .region_dir(oxidized_world::storage::Dimension::Overworld);

    // Group chunks by region coordinates.
    let mut by_region: HashMap<(i32, i32), Vec<ChunkPos>> = HashMap::new();
    for pos in &dirty {
        let rx = pos.x.div_euclid(32);
        let rz = pos.z.div_euclid(32);
        by_region.entry((rx, rz)).or_default().push(*pos);
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);

    let mut saved = 0usize;
    for ((rx, rz), positions) in &by_region {
        let region_path = region_dir.join(format!("r.{rx}.{rz}.mca"));
        let mut region = if region_path.exists() {
            RegionFile::open_rw(&region_path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
        } else {
            RegionFile::create(&region_path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
        };

        for pos in positions {
            let chunk_ref = match ctx.world.chunks.get(pos) {
                Some(r) => r.clone(),
                None => continue, // Chunk was unloaded since marking dirty
            };
            let chunk = chunk_ref.read();
            let nbt_bytes = match ctx.world.chunk_serializer.serialize(&chunk) {
                Ok(bytes) => bytes,
                Err(e) => {
                    warn!(pos = ?pos, error = %e, "Failed to serialize chunk, will retry");
                    continue;
                },
            };
            let compressed = match compress_zlib(&nbt_bytes) {
                Ok(data) => data,
                Err(e) => {
                    warn!(pos = ?pos, error = %e, "Failed to compress chunk, will retry");
                    continue;
                },
            };
            match region.write_chunk_data(pos.x, pos.z, &compressed, timestamp) {
                Ok(()) => {
                    // Only remove from dirty set after successful write.
                    ctx.world.dirty_chunks.remove(pos);
                    saved += 1;
                },
                Err(e) => {
                    warn!(pos = ?pos, error = %e, "Failed to write chunk, will retry");
                },
            }
        }
    }

    Ok(saved)
}

/// Autosaves dirty chunks from the tick thread, logging the outcome.
fn autosave_chunks(ctx: &ServerContext) {
    if ctx.world.dirty_chunks.is_empty() {
        return;
    }
    debug!("Autosaving dirty chunks...");
    match save_dirty_chunks_blocking(ctx) {
        Ok(count) => debug!(count, "Chunk autosave complete"),
        Err(e) => warn!(error = %e, "Chunk autosave failed"),
    }
}

/// Saves dirty chunks asynchronously via `spawn_blocking` (ADR-015).
///
/// Used by the shutdown save path (which runs in async context).
/// Serializes all dirty chunks in the current thread, then writes
/// them to region files on a blocking thread.
pub async fn save_dirty_chunks(
    ctx: &ServerContext,
) -> Result<usize, Box<dyn std::error::Error + Send>> {
    let dirty: Vec<ChunkPos> = ctx.world.dirty_chunks.iter().map(|r| *r).collect();
    if dirty.is_empty() {
        return Ok(0);
    }

    let region_dir = ctx
        .world
        .storage
        .region_dir(oxidized_world::storage::Dimension::Overworld);
    let serializer = ctx.world.chunk_serializer.clone();

    // Serialize all dirty chunks, skipping failures (they stay dirty).
    let mut chunk_data: Vec<(ChunkPos, Vec<u8>)> = Vec::with_capacity(dirty.len());
    for pos in &dirty {
        let chunk_ref = match ctx.world.chunks.get(pos) {
            Some(r) => r.clone(),
            None => continue,
        };
        let chunk = chunk_ref.read();
        let nbt_bytes = match serializer.serialize(&chunk) {
            Ok(b) => b,
            Err(e) => {
                warn!(pos = ?pos, error = %e, "Failed to serialize chunk on shutdown");
                continue;
            },
        };
        match compress_zlib(&nbt_bytes) {
            Ok(compressed) => chunk_data.push((*pos, compressed)),
            Err(e) => {
                warn!(pos = ?pos, error = %e, "Failed to compress chunk on shutdown");
            },
        }
    }

    // Track which positions we successfully serialized so we can
    // remove them from dirty_chunks after the disk write.
    let serialized_positions: Vec<ChunkPos> = chunk_data.iter().map(|(pos, _)| *pos).collect();

    let result = tokio::task::spawn_blocking(
        move || -> Result<usize, Box<dyn std::error::Error + Send>> {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as u32)
                .unwrap_or(0);

            type RegionChunks = HashMap<(i32, i32), Vec<(ChunkPos, Vec<u8>)>>;
            let mut by_region: RegionChunks = HashMap::new();
            for (pos, data) in chunk_data {
                let rx = pos.x.div_euclid(32);
                let rz = pos.z.div_euclid(32);
                by_region.entry((rx, rz)).or_default().push((pos, data));
            }

            let mut saved = 0usize;
            for ((rx, rz), chunks) in &by_region {
                let region_path = region_dir.join(format!("r.{rx}.{rz}.mca"));
                let mut region = if region_path.exists() {
                    RegionFile::open_rw(&region_path)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
                } else {
                    RegionFile::create(&region_path)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
                };
                for (pos, compressed) in chunks {
                    region
                        .write_chunk_data(pos.x, pos.z, compressed, timestamp)
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                    saved += 1;
                }
            }
            Ok(saved)
        },
    )
    .await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)??;

    // Remove successfully saved chunks from the dirty set.
    for pos in &serialized_positions {
        ctx.world.dirty_chunks.remove(pos);
    }

    Ok(result)
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
    use std::sync::Arc;

    const TICKS_PER_DAY: i64 = 24_000;

    fn test_ctx() -> Arc<ServerContext> {
        use oxidized_world::anvil::{AnvilChunkLoader, AsyncChunkLoader, ChunkSerializer};
        use oxidized_world::registry::BlockRegistry;
        use tokio::sync::broadcast;

        let block_registry = Arc::new(BlockRegistry::load().unwrap());
        let loader = AnvilChunkLoader::new(std::path::Path::new(""), block_registry.clone());
        Arc::new(ServerContext {
            world: crate::network::WorldContext {
                level_data: RwLock::new(
                    PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
                ),
                dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
                chunks: dashmap::DashMap::new(),
                dirty_chunks: dashmap::DashSet::new(),
                storage: LevelStorageSource::new(""),
                block_registry: block_registry.clone(),
                chunk_generator: Arc::new(oxidized_game::worldgen::flat::FlatChunkGenerator::new(
                    oxidized_game::worldgen::flat::FlatWorldConfig::default(),
                )),
                chunk_loader: Arc::new(AsyncChunkLoader::new(loader)),
                chunk_serializer: Arc::new(ChunkSerializer::new(block_registry)),
                game_rules: RwLock::new(GameRules::default()),
                pending_light_updates: parking_lot::Mutex::new(Vec::new()),
            },
            network: crate::network::NetworkContext {
                broadcast_tx: broadcast::channel(256).0,
                shutdown_tx: broadcast::channel(1).0,
                kick_channels: dashmap::DashMap::new(),
                player_list: RwLock::new(PlayerList::new(20)),
                max_players: 20,
            },
            settings: crate::network::ServerSettings {
                max_view_distance: 10,
                max_simulation_distance: 10,
                op_permission_level: 4,
                spawn_protection: 16,
                color_char: None,
                timeouts: crate::config::NetworkTimeoutsConfig::default(),
                connection_rate_limit: crate::config::RateLimitConfig::default(),
                entity_tracking: crate::config::EntityTrackingConfig::default(),
                weather: crate::config::WeatherConfig::default(),
                inbound_channel_capacity: 128,
                outbound_channel_capacity: 512,
                chunk_cache_size: 1024,
                max_concurrent_chunk_generations: 64,
            },
            commands: oxidized_game::commands::Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            tick_rate_manager: RwLock::new(ServerTickRateManager::default()),
            ops: Arc::new(crate::ops::OpsStore::load("/dev/null/nonexistent", 4)),
            self_ref: std::sync::OnceLock::new(),
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

    #[test]
    fn test_do_tick_advances_time() {
        let ctx = test_ctx();
        let initial_time = ctx.world.level_data.read().time.game_time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather());
        assert_eq!(ctx.world.level_data.read().time.game_time, initial_time + 1);
    }

    #[test]
    fn test_do_tick_advances_day_time() {
        let ctx = test_ctx();
        let initial_day = ctx.world.level_data.read().time.day_time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather());
        assert_eq!(ctx.world.level_data.read().time.day_time, initial_day + 1);
    }

    #[test]
    fn test_day_time_grows_unbounded() {
        let ctx = test_ctx();
        ctx.world.level_data.write().time.day_time = TICKS_PER_DAY - 1;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather());
        assert_eq!(
            ctx.world.level_data.read().time.day_time,
            TICKS_PER_DAY,
            "day_time should grow past 24000 (client handles modulo)"
        );
    }

    #[test]
    fn test_daylight_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.world
            .game_rules
            .write()
            .set_bool(GameRuleKey::AdvanceTime, false);
        let initial_day = ctx.world.level_data.read().time.day_time;
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather());
        assert_eq!(
            ctx.world.level_data.read().time.day_time,
            initial_day,
            "day_time should not advance when advance_time is false"
        );
        // But game_time always advances.
        assert_eq!(ctx.world.level_data.read().time.game_time, 1);
    }

    #[test]
    fn test_weather_cycle_respects_gamerule() {
        let ctx = test_ctx();
        ctx.world
            .game_rules
            .write()
            .set_bool(GameRuleKey::AdvanceWeather, false);
        ctx.world.level_data.write().weather.rain_time = 1; // would flip without gamerule
        do_tick(&ctx, 0, &mut test_rng(), &mut test_weather());
        // Weather should not have changed because gamerule is false.
        assert_eq!(
            ctx.world.level_data.read().weather.rain_time,
            1,
            "weather should not tick when advance_weather is false"
        );
    }

    #[test]
    fn test_weather_transitions() {
        let ctx = test_ctx();
        ctx.world.level_data.write().weather.is_raining = false;
        ctx.world.level_data.write().weather.rain_time = 1;
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        assert!(
            ctx.world.level_data.read().weather.is_raining,
            "should start raining"
        );
    }

    #[test]
    fn test_thunder_independent_of_rain() {
        // In vanilla 26.1, thunder and rain are independent state machines.
        // Thunder doesn't require rain — both can toggle independently.
        let ctx = test_ctx();
        {
            let mut ld = ctx.world.level_data.write();
            ld.weather.is_raining = false;
            ld.weather.is_thundering = true;
            ld.weather.thunder_time = 5;
        }
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        // Thunder remains true because its timer hasn't reached 0.
        assert!(
            ctx.world.level_data.read().weather.is_thundering,
            "thunder should persist independently of rain"
        );
    }

    #[test]
    fn test_clear_weather_time_overrides() {
        let ctx = test_ctx();
        {
            let mut ld = ctx.world.level_data.write();
            ld.weather.is_raining = true;
            ld.weather.is_thundering = true;
            ld.weather.clear_weather_time = 100;
            ld.weather.rain_time = 5000;
            ld.weather.thunder_time = 3000;
        }
        tick_weather(&ctx, &mut test_rng(), &mut test_weather());
        let ld = ctx.world.level_data.read();
        assert!(!ld.weather.is_raining, "clear weather should stop rain");
        assert!(
            !ld.weather.is_thundering,
            "clear weather should stop thunder"
        );
        assert_eq!(
            ld.weather.clear_weather_time, 99,
            "clear weather timer should decrement"
        );
    }

    #[test]
    fn test_weather_random_duration_in_range() {
        let ctx = test_ctx();
        // Set rain_time to 0 while raining to trigger duration sample.
        {
            let mut ld = ctx.world.level_data.write();
            ld.weather.is_raining = true;
            ld.weather.rain_time = 0;
        }
        tick_weather(
            &ctx,
            &mut rand::rngs::SmallRng::seed_from_u64(12345),
            &mut test_weather(),
        );
        let rain_time = ctx.world.level_data.read().weather.rain_time;
        let w = &ctx.settings.weather;
        assert!(
            (w.rain_duration_min..=w.rain_duration_max).contains(&rain_time),
            "rain duration {rain_time} should be in range {}..={}",
            w.rain_duration_min,
            w.rain_duration_max
        );
    }

    #[test]
    fn test_tick_loop_shutdown() {
        let ctx = test_ctx();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = std::thread::Builder::new()
            .name("tick-test".into())
            .spawn(move || {
                let (_tx, rx) = oxidized_game::entity::commands::entity_command_channel(64);
                let ecs = crate::ecs::EcsContext::new(rx);
                run_tick_loop(&ctx, ecs, &shutdown_clone);
            })
            .expect("failed to spawn tick thread");

        // Let a few ticks run.
        std::thread::sleep(Duration::from_millis(120));
        shutdown.store(true, Ordering::Relaxed);

        handle.join().expect("tick thread panicked");
    }

    #[test]
    fn test_tick_loop_runs_on_named_thread() {
        use std::sync::Mutex;

        let ctx = test_ctx();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_name: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let shutdown_clone = shutdown.clone();
        let name_capture = thread_name.clone();

        let handle = std::thread::Builder::new()
            .name("tick".into())
            .spawn(move || {
                // Capture thread name before entering the loop.
                *name_capture.lock().unwrap() = std::thread::current().name().map(String::from);
                let (_tx, rx) = oxidized_game::entity::commands::entity_command_channel(64);
                let ecs = crate::ecs::EcsContext::new(rx);
                run_tick_loop(&ctx, ecs, &shutdown_clone);
            })
            .expect("failed to spawn tick thread");

        std::thread::sleep(Duration::from_millis(60));
        shutdown.store(true, Ordering::Relaxed);
        handle.join().expect("tick thread panicked");

        let name = thread_name.lock().unwrap();
        assert_eq!(
            name.as_deref(),
            Some("tick"),
            "tick loop must run on the 'tick' thread"
        );
    }
}
