# Phase 38 — Performance Hardening

**Status:** 📋 Planned  
**Crate:** all  
**Reward:** Server handles 100+ concurrent players without lag; watchdog
catches hangs; crashes produce actionable reports.

**Depends on:** All previous phases. This phase adds no new features; it
hardens, instruments, and optimises the entire stack.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-001: Async Runtime](../adr/adr-001-async-runtime.md) — Tokio runtime selection and async patterns
- [ADR-004: Logging & Observability](../adr/adr-004-logging-observability.md) — tracing with structured spans and metrics
- [ADR-006: Network I/O](../adr/adr-006-network-io.md) — per-connection task pairs with mpsc channels
- [ADR-014: Chunk Storage](../adr/adr-014-chunk-storage.md) — DashMap + per-section RwLock for concurrent access
- [ADR-019: Tick Loop](../adr/adr-019-tick-loop.md) — parallel tick phases with ECS system scheduling
- [ADR-029: Memory Management](../adr/adr-029-memory-management.md) — mimalloc + arena allocation + buffer pooling
- [ADR-030: Shutdown & Crash Handling](../adr/adr-030-shutdown-crash.md) — multi-layer shutdown with watchdog and crash reports
- [ADR-032: Scalability](../adr/adr-032-scalability.md) — multi-layered scalability: DOD, parallel ECS, smart scheduling


## Goal

Take a functionally complete server and make it production-ready for 100+
concurrent players. This involves per-connection rate limiting, intelligent
chunk eviction, async entity tracking, a watchdog thread, crash reporting,
metrics collection, and careful tick-loop budgeting.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Main server loop | `MinecraftServer` | `net.minecraft.server.MinecraftServer` |
| Tick rate manager | `ServerTickRateManager` | `net.minecraft.server.ServerTickRateManager` |
| Watchdog thread | `WatchdogThread` | `net.minecraft.server.WatchdogThread` |
| Crash report | `CrashReport` | `net.minecraft.CrashReport` |
| Crash report category | `CrashReportCategory` | `net.minecraft.CrashReportCategory` |
| Server tick list | `ServerLevel#tick` | `net.minecraft.server.level.ServerLevel` |
| Entity tracker | `ChunkMap.TrackedEntity` | `net.minecraft.server.level.ChunkMap` |

---

## Tasks

### 38.1 — Per-connection packet rate limiting
(`oxidized-protocol/src/connection/rate_limit.rs`)

Prevents malicious or buggy clients from flooding the server:

```rust
pub struct PacketRateLimiter {
    /// Ring buffer of packet timestamps (nanoseconds).
    window: VecDeque<u64>,
    /// Maximum packets allowed per tick window (20-tick sliding window).
    max_per_window: usize,
}

const WINDOW_TICKS: u64 = 20;
const MAX_PACKETS_PER_WINDOW: usize = 500;

impl PacketRateLimiter {
    pub fn check(&mut self, now_ns: u64) -> bool {
        let window_ns = WINDOW_TICKS * 50_000_000; // 20 ticks * 50ms in ns
        // Drop packets outside the window
        while self.window.front().is_some_and(|&t| now_ns - t > window_ns) {
            self.window.pop_front();
        }
        if self.window.len() >= self.max_per_window {
            return false; // rate exceeded → disconnect client
        }
        self.window.push_back(now_ns);
        true
    }
}
```

- [ ] Integrate into `PacketReader` loop in `oxidized-protocol`
- [ ] Disconnect with `"Kicked for flooding"` message on violation
- [ ] Log the violation at `WARN` level with remote address

### 38.2 — Entity culling and metadata batching
(`oxidized-game/src/entity/tracker.rs`)

Don't send entity metadata updates for entities outside a player's tracking
range:

```rust
pub const ENTITY_TRACKING_RANGES: &[(EntityType, i32)] = &[
    (EntityType::Player,     128),
    (EntityType::Animal,      64),
    (EntityType::Monster,     64),
    (EntityType::Merchant,    64),
    (EntityType::Misc,        32),
    (EntityType::Other,       16),
];

pub struct TrackedEntity {
    pub entity_id: i32,
    pub entity_type: EntityType,
    pub tracking_range: i32,
    /// Players currently tracking this entity.
    pub viewers: HashSet<Uuid>,
    pub last_sent_pos: Vec3,
    pub dirty: bool,
}
```

- [ ] On each tick, batch all dirty entities and compute per-player visibility
- [ ] Send `ClientboundAddEntityPacket` when an entity enters a player's range
- [ ] Send `ClientboundRemoveEntitiesPacket` when it leaves
- [ ] Skip metadata updates for non-visible entities entirely

### 38.3 — Per-player view distance
(`oxidized-game/src/player/view_distance.rs`)

```rust
pub fn set_view_distance(player: &mut ServerPlayer, distance: u8, level: &mut ServerLevel) {
    let distance = distance.clamp(1, 32);
    let old = player.view_distance;
    player.view_distance = distance;
    // Unload chunks no longer in range
    for chunk_pos in chunks_in_range(player.chunk_pos, old)
        .filter(|c| !chunks_in_range(player.chunk_pos, distance).contains(c))
    {
        player.connection.send(ClientboundForgetLevelChunkPacket { chunk_pos });
    }
    // Load chunks newly in range
    for chunk_pos in chunks_in_range(player.chunk_pos, distance)
        .filter(|c| !chunks_in_range(player.chunk_pos, old).contains(c))
    {
        level.chunk_sender.schedule(chunk_pos, Priority::Player);
    }
}
```

- [ ] `/view-distance <n>` command sets per-player distance (Phase 18 extension)
- [ ] Default from `view-distance` in `oxidized.toml`
- [ ] Range: 1–32 chunks

### 38.4 — Chunk worker pool and priorities
(`oxidized-world/src/chunk/loader.rs`)

```rust
pub enum ChunkPriority {
    /// Player is about to see this chunk.
    Player,
    /// Chunk needed for entity simulation but not visible.
    Simulation,
    /// Background save/generation.
    Background,
}

pub struct ChunkWorkerPool {
    /// High-priority queue: player-needed chunks.
    player_queue: Arc<Mutex<BinaryHeap<PrioritizedChunk>>>,
    /// Low-priority queue: background worldgen + I/O.
    background_queue: Arc<Mutex<BinaryHeap<PrioritizedChunk>>>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}
```

- [ ] Use `tokio::task::spawn_blocking` for disk I/O (region file reads/writes)
- [ ] Prioritise chunks adjacent to players over background worldgen
- [ ] Worker count: `min(cpu_count, 8)` blocking threads

### 38.5 — Chunk LRU eviction
(`oxidized-world/src/chunk/cache.rs`)

```rust
pub struct ChunkCache {
    loaded: LinkedHashMap<ChunkPos, Arc<RwLock<LevelChunk>>>,
    max_loaded: usize,
}

impl ChunkCache {
    /// Compute max_loaded from game settings.
    pub fn capacity(view_distance: u8, max_players: u32) -> usize {
        let vd = view_distance as usize;
        let per_player = (2 * vd + 1).pow(2);
        ((per_player * max_players as usize) as f64 * 1.5) as usize
    }

    pub fn touch(&mut self, pos: ChunkPos) {
        // Move to back (most recently used)
        if let Some(chunk) = self.loaded.remove(&pos) {
            self.loaded.insert(pos, chunk);
        }
    }

    pub fn evict_lru(&mut self, io_sender: &IoSender) {
        while self.loaded.len() > self.max_loaded {
            let (pos, chunk) = self.loaded.pop_front().unwrap();
            let chunk = chunk.read().unwrap();
            if chunk.is_dirty() {
                io_sender.send_save(pos, chunk.clone());
            }
        }
    }
}
```

### 38.6 — Tick loop optimisation

```rust
// crates/oxidized-game/src/server/tick.rs

pub fn tick_level(level: &mut ServerLevel) {
    // 1. Skip entirely empty sections (no entities, no random ticks)
    for section in level.chunk_sections_mut() {
        if section.is_empty() { continue; }
        section.tick_random_blocks(&mut level.rng);
    }

    // 2. Skip AI for entities in unloaded chunks
    for entity in level.entities_mut() {
        if !level.is_chunk_loaded(entity.chunk_pos()) { continue; }
        entity.tick(level);
    }

    // 3. Batch packet flush: collect all outgoing packets, flush once per tick
    for player in level.players_mut() {
        player.connection.flush_pending().await;
    }
}
```

### 38.7 — Network batching (flush once per tick)

```rust
// crates/oxidized-protocol/src/connection/writer.rs

pub struct PacketWriter {
    inner: tokio::io::BufWriter<TcpStream>,
    pending: Vec<Bytes>,
}

impl PacketWriter {
    pub fn queue(&mut self, packet: impl Encode) {
        let mut buf = BytesMut::new();
        packet.encode(&mut buf);
        self.pending.push(buf.freeze());
    }

    /// Called once per tick, not per packet.
    pub async fn flush_all(&mut self) -> io::Result<()> {
        for pkt in self.pending.drain(..) {
            self.inner.write_all(&pkt).await?;
        }
        self.inner.flush().await
    }
}
```

### 38.8 — Keepalive (`oxidized-game/src/player/keepalive.rs`)

```rust
pub const KEEPALIVE_INTERVAL_TICKS: u64 = 15;
pub const KEEPALIVE_TIMEOUT_TICKS: u64 = 600; // 30s = 600 ticks at 20 TPS

pub struct KeepaliveState {
    last_sent_id: i64,
    last_sent_tick: u64,
    waiting_for_response: bool,
}

pub fn tick_keepalive(state: &mut KeepaliveState, player: &mut PlayerConnection, tick: u64) {
    if state.waiting_for_response {
        if tick - state.last_sent_tick > KEEPALIVE_TIMEOUT_TICKS {
            player.disconnect("Timed out");
        }
        return;
    }
    if tick % KEEPALIVE_INTERVAL_TICKS == 0 {
        let id = tick as i64;
        player.send(ClientboundKeepAlivePacket { id });
        state.last_sent_id = id;
        state.last_sent_tick = tick;
        state.waiting_for_response = true;
    }
}

pub fn on_keepalive_response(state: &mut KeepaliveState, id: i64) -> bool {
    if id == state.last_sent_id {
        state.waiting_for_response = false;
        true
    } else {
        false // mismatch → disconnect
    }
}
```

### 38.9 — `ServerTickRateManager`
(`oxidized-server/src/tick_rate.rs`)

```rust
pub struct ServerTickRateManager {
    pub target_tps: f32,          // Normal: 20.0
    pub sprint_remaining: u32,    // Ticks to run as fast as possible
    pub paused: bool,             // Stop ticking when no players
    pub mspt_budget: u64,         // Nanoseconds per tick (50_000_000 at 20 TPS)
}

impl ServerTickRateManager {
    pub fn set_tps(&mut self, tps: f32) {
        self.target_tps = tps.clamp(1.0, 10000.0);
        self.mspt_budget = (1_000_000_000.0 / self.target_tps) as u64;
    }

    pub fn tick_sprint(&mut self, ticks: u32) {
        self.sprint_remaining = ticks;
    }

    pub fn should_skip_sleep(&self) -> bool {
        self.sprint_remaining > 0
    }

    pub fn after_tick(&mut self) {
        if self.sprint_remaining > 0 { self.sprint_remaining -= 1; }
    }
}
```

Pausing (`pause_when_empty_seconds`): when no players have been online for the
configured seconds, set `paused = true`; resume immediately on player join.

### 38.10 — Watchdog thread
(`oxidized-server/src/watchdog.rs`)

```rust
pub const WATCHDOG_REPORT_MS: u64 = 10_000;  // Log "Can't keep up" after 10s behind
pub const WATCHDOG_CRASH_MS: u64  = 60_000;  // Crash report after 60s no tick

pub struct Watchdog {
    last_tick_time: Arc<AtomicU64>, // updated each tick
    max_tick_time_ms: u64,
}

impl Watchdog {
    pub fn start(self) {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(5));
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
            let last = self.last_tick_time.load(Ordering::Relaxed);
            let lag_ms = now.saturating_sub(last);
            if lag_ms > WATCHDOG_CRASH_MS {
                generate_crash_report("Watchdog: server appears frozen");
                std::process::abort();
            } else if lag_ms > WATCHDOG_REPORT_MS {
                tracing::warn!(
                    "Can't keep up! Is the server overloaded? \
                     Running {}ms ({} ticks) behind",
                    lag_ms, lag_ms / 50
                );
            }
        });
    }
}
```

### 38.11 — Crash report (`oxidized-server/src/crash.rs`)

On panic (via `std::panic::set_hook`) or watchdog trigger:

```
---- Minecraft Crash Report ----
// <random witty comment>

Time: 2026-04-01T12:00:00Z
Description: Ticking server

net.minecraft.server equivalent panic:
  thread 'tokio-runtime-worker' panicked at 'index out of bounds', src/...

-- System Details --
Minecraft Version: 26.1-pre-3 (protocol 1073742124)
Operating System: Linux 6.8.0 (amd64)
CPU: 8 × Intel(R) Core(TM) i7
Memory: 6144MB used / 16384MB total
JVM Flags: N/A (Rust binary)

-- Loaded Worlds --
  overworld: 42 loaded chunks, 1024 entities
  the_nether: 0 loaded chunks, 0 entities

-- Online Players --
  Steve (uuid) at (128.5, 64.0, -32.3)
```

- [ ] Write to `crash-reports/crash-<timestamp>.txt`
- [ ] Log the file path to stderr
- [ ] Include last 20 tick durations in milliseconds

### 38.12 — `--forceUpgrade` flag

CLI argument that, when passed, iterates all region files in all dimensions and
upgrades each chunk to the current data version before starting normal
operation.

```rust
// crates/oxidized-server/src/main.rs

if args.force_upgrade {
    tracing::info!("Force-upgrading all chunks...");
    for dimension in &["overworld", "the_nether", "the_end"] {
        upgrade_dimension(&world_dir.join(dimension), current_data_version);
    }
    tracing::info!("Force-upgrade complete.");
}
```

### 38.13 — IPv6 support

```rust
let bind_addr: SocketAddr = if server_ip.is_empty() {
    // Bind to all interfaces, prefer IPv6 dual-stack
    SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))
} else {
    format!("{server_ip}:{port}").parse()?
};
```

Set `IPV6_V6ONLY(false)` so a `[::]` bind also accepts IPv4 connections on
dual-stack systems.

### 38.14 — Metrics collection
(`oxidized-server/src/metrics.rs`)

```rust
pub struct ServerMetrics {
    /// Rolling TPS history: last 20s, 5min, 15min averages.
    tps_samples: CircularBuffer<f64, 18000>, // 15 min × 20 TPS
    /// Memory usage snapshots (RSS in bytes).
    memory_samples: CircularBuffer<u64, 300>, // 5 min
    /// Chunk counts per dimension.
    chunk_stats: HashMap<DimensionType, ChunkStats>,
}

impl ServerMetrics {
    pub fn tps_1m(&self) -> f64  { self.average_tps(20 * 60) }
    pub fn tps_5m(&self) -> f64  { self.average_tps(20 * 300) }
    pub fn tps_15m(&self) -> f64 { self.average_tps(20 * 900) }
}
```

Expose via `/tps` command:
```
[Server] TPS: 20.0 (last 1m) / 19.8 (last 5m) / 19.5 (last 15m)
```

---

## Acceptance Criteria

- [ ] 100 simulated clients connecting simultaneously — server stays at ≥ 18 TPS
- [ ] A client sending 1000 packets/tick is disconnected with "Kicked for
      flooding"
- [ ] Watchdog logs "Can't keep up" when the tick loop falls > 10s behind
- [ ] Watchdog writes a crash report and exits if the server freezes > 60s
- [ ] `/tps` returns TPS history across 1m / 5m / 15m windows
- [ ] `--forceUpgrade` completes without data corruption
- [ ] Server starts on `[::]` and accepts both IPv4 and IPv6 connections when
      `server-ip` is blank
- [ ] Crash report is written to `crash-reports/` with system info and player
      list on panic
- [ ] Memory usage stays below 4 GB with 100 players and a 10-chunk view
      distance (default world, no worldgen)
