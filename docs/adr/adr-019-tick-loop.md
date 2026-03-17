# ADR-019: Server Tick Loop Design

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P19, P38 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's game simulation advances in discrete steps called "ticks," running at a fixed
rate of 20 ticks per second (one tick every 50 milliseconds). Every game mechanic — entity
movement, block updates, redstone propagation, mob AI, weather, daylight cycle, crop
growth, furnace smelting — is driven by this tick clock. The tick loop is the heartbeat of
the server; its design determines the server's throughput ceiling, latency
characteristics, and scalability limits.

Vanilla Minecraft runs its tick loop on a single thread. The `MinecraftServer.tickServer()`
method executes sequentially: process incoming network packets, tick each loaded dimension
(which ticks all loaded chunks, which ticks all entities in those chunks), then flush
outgoing network packets. If any single tick takes longer than 50ms, the server falls
behind the target tick rate. When this happens, vanilla logs "Can't keep up!" and attempts
to catch up by running ticks back-to-back without sleeping. This single-threaded design
means a server with 500 entities in the Overworld and 500 in the Nether cannot use two CPU
cores — it must tick all 1000 entities sequentially. This is the primary reason vanilla
servers struggle above ~200 concurrent players.

Oxidized's entity system (ADR-018) uses `bevy_ecs`, which provides automatic parallel
system scheduling. However, the tick loop itself must orchestrate when ECS systems run,
how dimensions interact, how network I/O integrates with the game simulation, and how to
handle the case where a tick exceeds its 50ms budget. The tick loop must also support the
`/tick` command family (freeze, step, sprint, rate) introduced in Minecraft 24w04a. The
design must balance maximum parallelism with deterministic behavior — game state after N
ticks from identical initial conditions must be identical regardless of thread scheduling.

## Decision Drivers

- **Maximize CPU utilization**: On an 8-core server, the tick loop should be able to use
  all 8 cores for entity processing, block ticks, and other parallelizable work.
- **Maintain tick determinism**: Given identical initial state and identical input (packets),
  the game state after N ticks must be identical. Non-determinism from thread scheduling
  would break redstone contraptions, mob farms, and player expectations.
- **50ms tick budget enforcement**: The server must detect when ticks exceed budget, log
  warnings, and handle catch-up gracefully without causing cascading lag.
- **Clean phase separation**: Network I/O, game simulation, and world persistence must not
  interleave within a single tick. Systems must be able to assume stable state during their
  execution.
- **Support `/tick` commands**: Tick rate control (freeze, step N, sprint, rate N) must be
  first-class, not bolted on.
- **Watchdog safety**: A stuck tick (infinite loop in a system, deadlock) must be detected
  and the server shut down cleanly rather than hanging forever.

## Considered Options

### Option 1: Single-Threaded Like Vanilla

Replicate vanilla's sequential tick loop exactly. One thread ticks everything in order.

**Pros:**
- Trivially deterministic — no concurrency concerns.
- Simplest to implement and debug.
- Easiest to match vanilla behavior (same execution order).

**Cons:**
- Wastes all but one CPU core. On modern 8-16 core servers, this is unacceptable.
- The #1 performance problem in vanilla remains unsolved.
- Defeats the purpose of choosing ECS (ADR-018) — bevy_ecs's parallel scheduling is unused.

**Verdict: Rejected.** If we wanted single-threaded, we wouldn't need ECS.

### Option 2: Multi-Threaded Tick with Phase Barriers

Divide each tick into sequential phases. Within each phase, work is parallelized. A barrier
between phases ensures all work in phase N completes before phase N+1 begins.

**Pros:**
- Deterministic — phase ordering is fixed, and within a phase, systems that don't conflict
  run in parallel but produce the same result regardless of scheduling.
- Leverages all CPU cores within each phase.
- Natural fit for bevy_ecs's parallel system scheduler.
- Phase barriers provide clear points for consistency checks and network sync.

**Cons:**
- Phase barriers introduce synchronization overhead (thread join/notify).
- If one phase has a long-running system, all other threads wait at the barrier.
- Requires careful system assignment to phases to maximize parallelism.

**Verdict: Selected.** Best balance of parallelism and determinism.

### Option 3: Per-Dimension Threading

Each dimension (Overworld, Nether, End) runs on its own thread with its own tick loop.
Cross-dimension interactions (portals, commands) use message passing.

**Pros:**
- Simple isolation — each dimension is independent.
- Scales linearly with the number of loaded dimensions.

**Cons:**
- Most servers have 90%+ of activity in the Overworld. Per-dimension threading doesn't
  help the most loaded dimension at all.
- Cross-dimension interactions (portal travel, `/tp`, end portal, respawn anchor) require
  complex synchronization and message passing.
- Entity transfer between dimensions requires careful handoff.
- Does not help within a single dimension — 10,000 entities in the Overworld are still
  single-threaded.

**Verdict: Rejected.** Doesn't solve the common case (single-dimension bottleneck).

### Option 4: Full ECS Parallel Systems (Bevy-Style)

Let bevy_ecs schedule all systems across all threads with no explicit phases. Systems
declare dependencies and bevy_ecs figures out the execution order.

**Pros:**
- Maximum theoretical parallelism — the scheduler finds every opportunity.
- No manual phase assignment.

**Cons:**
- Determinism is not guaranteed — system execution order within a frame depends on the
  scheduler's decisions, which may vary between runs.
- Harder to reason about when network I/O happens relative to game state changes.
- Debugging system ordering issues is extremely difficult.
- `/tick step` and `/tick freeze` are harder to implement without explicit phase control.

**Verdict: Rejected.** Determinism is non-negotiable for a Minecraft server.

### Option 5: Hybrid — ECS Parallel Within Dimensions, Dimensions Serial

Tick dimensions in sequence (Overworld → Nether → End), but within each dimension, use
bevy_ecs parallel system scheduling.

**Pros:**
- No cross-dimension synchronization needed during a tick.
- ECS parallelism within the hot path (single dimension).

**Cons:**
- Dimensions tick serially, wasting potential parallelism when multiple dimensions are
  loaded with significant activity.
- Portal interactions are simpler (dimension hasn't been ticked yet or has already been
  ticked) but the ordering matters and must match vanilla.

**Verdict: Partially adopted.** We use this as the default but allow dimensions to tick in
parallel when no cross-dimension interactions are pending (an optimization for later phases).

## Decision

**The tick loop uses ECS-driven parallel execution with explicit phase barriers.** Each
server tick executes a fixed sequence of phases. Within each phase, `bevy_ecs` schedules
systems for parallel execution. Phase barriers (thread synchronization points) separate
each phase, ensuring all work in one phase completes before the next begins. This provides
determinism (fixed phase order) with parallelism (multi-threaded within phases).

### Tick Phase Sequence

```
┌─────────────────────────────────────────────┐
│               Server Tick N                 │
├─────────────────────────────────────────────┤
│ Phase 1: NETWORK_RECEIVE                    │
│   - Drain all queued inbound packets        │
│   - Deserialize and validate                │
│   - Convert to ECS events/commands          │
│   ─── barrier ───                           │
│ Phase 2: WORLD_TICK                         │
│   - Advance game time (daylight cycle)      │
│   - Weather state machine                   │
│   - Scheduled tick processing               │
│   - Raid/patrol spawning                    │
│   - (per dimension, parallelizable)         │
│   ─── barrier ───                           │
│ Phase 3: ENTITY_TICK                        │
│   - Physics: gravity, velocity, collision   │
│   - AI: goal evaluation, pathfinding        │
│   - Entity behavior: type-specific logic    │
│   - Status effects: apply, tick, expire     │
│   - (ECS parallel across all systems)       │
│   ─── barrier ───                           │
│ Phase 4: BLOCK_TICK                         │
│   - Random block ticks (crop growth, etc.)  │
│   - Scheduled block ticks (redstone, etc.)  │
│   - Block entity ticks (furnaces, hoppers)  │
│   ─── barrier ───                           │
│ Phase 5: NETWORK_SEND                       │
│   - Serialize dirty entity data             │
│   - Chunk updates, block changes            │
│   - Flush outbound packet queues            │
│   ─── barrier ───                           │
│ Phase 6: HOUSEKEEPING                       │
│   - Auto-save check (every 6000 ticks)      │
│   - Player list ping update                 │
│   - TPS measurement update                  │
│   - Chunk ticket expiry                     │
│   - Garbage collection hint (if needed)     │
└─────────────────────────────────────────────┘
```

### Tick Timing

The tick loop uses `tokio::time::interval(Duration::from_millis(50))` with
`MissedTickBehavior::Burst`. This means:

- If a tick completes in < 50ms, the loop sleeps until the next tick boundary.
- If a tick takes 50-100ms, the next tick starts immediately (no sleep) to catch up.
- If the server falls behind by > 10 ticks (500ms), it logs "Can't keep up! Is the server
  overloaded?" and skips ticks to prevent a cascade (resets the interval). This matches
  vanilla's catch-up behavior.

```rust
let mut tick_interval = tokio::time::interval(Duration::from_millis(50));
tick_interval.set_missed_tick_behavior(MissedTickBehavior::Burst);

let mut tick_count: u64 = 0;
let mut last_overload_warning = Instant::now();

loop {
    tick_interval.tick().await;

    let tick_start = Instant::now();
    server.run_tick(tick_count);
    let tick_duration = tick_start.elapsed();

    if tick_duration > Duration::from_millis(50) {
        let behind_ms = tick_duration.as_millis() - 50;
        if behind_ms > 500 && last_overload_warning.elapsed() > Duration::from_secs(15) {
            warn!("Can't keep up! Is the server overloaded? Running {}ms behind", behind_ms);
            last_overload_warning = Instant::now();
            tick_interval.reset();
        }
    }

    tick_count += 1;
}
```

### Tick Rate Control (`/tick` Command)

The tick rate manager is a resource in the ECS world:

```rust
#[derive(Resource)]
struct TickRateManager {
    target_rate: f32,         // default 20.0
    frozen: bool,             // /tick freeze
    stepping: Option<u32>,    // /tick step N — remaining steps
    sprinting: Option<SprintState>, // /tick sprint N
}
```

When `frozen` is `true`, the tick loop skips all phases except `NETWORK_RECEIVE` (so
players can still send commands to unfreeze). When `stepping` is `Some(n)`, exactly `n`
ticks execute then re-freeze. When `sprinting`, ticks run as fast as possible (no 50ms
sleep) for `n` ticks, then resume normal rate.

### Watchdog Thread

A dedicated watchdog thread monitors tick duration:

```rust
fn watchdog_thread(last_tick_start: Arc<AtomicU64>, timeout: Duration) {
    loop {
        thread::sleep(timeout); // default: 60 seconds
        let started = last_tick_start.load(Ordering::Relaxed);
        let elapsed = Instant::now().duration_since(/* decode started */);
        if elapsed > timeout {
            error!("A single server tick took {:.1}s (should be max 0.05s)", elapsed.as_secs_f64());
            error!("Consider reducing view-distance, max entities, or installed mods");
            // Dump all thread stack traces for diagnosis
            std::process::abort();
        }
    }
}
```

The watchdog timeout is configurable (default 60s). It can be disabled for debugging. This
matches vanilla's `ServerWatchdog` behavior.

### TPS Measurement

The server tracks a rolling average of tick durations:

```rust
#[derive(Resource)]
struct TickTimings {
    recent_ticks: VecDeque<Duration>, // last 100 tick durations
    tps_1s: f64,   // TPS over last 1 second (20 ticks)
    tps_5s: f64,   // TPS over last 5 seconds (100 ticks)
    tps_1m: f64,   // TPS over last 60 seconds (1200 ticks)
    mspt: f64,     // average milliseconds per tick (last 100)
}
```

Exposed via the `/tick query` command and available to monitoring systems.

### Game Rule Interaction

Game rules that affect tick behavior are read once at the start of the relevant phase and
cached for that tick. Examples:
- `doDaylightCycle` — checked at start of WORLD_TICK; if false, skip time advancement.
- `doMobSpawning` — checked at start of ENTITY_TICK; if false, skip spawn cycle.
- `randomTickSpeed` — read at start of BLOCK_TICK; determines random tick count per chunk
  section (default 3).

## Consequences

### Positive

- **Multi-core utilization**: The ENTITY_TICK phase alone can saturate all CPU cores when
  processing thousands of entities with independent systems.
- **Deterministic behavior**: Phase barriers ensure that all entities complete physics
  before any entity starts AI. This matches vanilla's effective ordering and prevents
  non-deterministic behavior.
- **Clean debugging**: Each phase is a well-defined unit. Performance profiling can show
  exactly which phase is the bottleneck. `/tick` commands provide runtime introspection.
- **Graceful degradation**: The catch-up mechanism and watchdog ensure the server handles
  overload predictably rather than silently falling behind.

### Negative

- **Phase barrier overhead**: Synchronizing threads at phase boundaries has a cost
  (typically 1-10μs per barrier). With 6 phases per tick, this adds ~6-60μs per tick —
  negligible compared to the 50ms budget but measurable in profiling.
- **Uneven phase load**: If ENTITY_TICK takes 40ms and BLOCK_TICK takes 2ms, the
  parallelism within BLOCK_TICK doesn't help. The longest phase determines the minimum
  tick duration.
- **Cross-phase dependencies**: Some vanilla mechanics span what we've split into separate
  phases. A block update that spawns an entity (mob spawner) needs deferred entity creation
  via `Commands`, processed at the next phase barrier.

### Neutral

- **Dimension parallelism is an optimization target**: Initially, dimensions tick
  sequentially within each phase. A future optimization can run dimensions in parallel
  within WORLD_TICK and BLOCK_TICK phases, since these are per-dimension operations.
- **Tokio integration**: The tick loop runs on a dedicated thread (not a tokio task) to
  avoid interference from async I/O. Network I/O runs on tokio's runtime. The phases
  bridge between the two via channels.

## Compliance

- **Phase ordering test**: An integration test records the order in which systems execute
  and asserts it matches the defined phase sequence.
- **Determinism test**: Run two identical server instances with the same seed, same player
  inputs. After 1000 ticks, assert world state is byte-identical.
- **TPS measurement accuracy**: Unit test that a no-op tick loop achieves exactly 20 TPS
  (within ±0.5 TPS tolerance) over a 5-second measurement window.
- **Watchdog test**: Inject an artificial 65-second delay in a tick and verify the watchdog
  triggers within 5 seconds of the timeout threshold.
- **Catch-up test**: Inject 3 consecutive 100ms ticks and verify the server catches up
  within 10 ticks (runs ticks back-to-back until caught up).

## Related ADRs

- **ADR-018**: Entity System Architecture — defines the ECS systems scheduled within
  ENTITY_TICK
- **ADR-020**: Player Session Lifecycle — network tasks feed packets into NETWORK_RECEIVE
  and consume from NETWORK_SEND
- **ADR-021**: Physics & Collision Engine — physics systems run in the ENTITY_TICK phase
- **ADR-023**: AI & Pathfinding — AI systems run in the ENTITY_TICK phase after physics
- **ADR-025**: Redstone Simulation — scheduled ticks processed in BLOCK_TICK phase

## References

- Vanilla source: `net.minecraft.server.MinecraftServer.tickServer()`
- Vanilla source: `net.minecraft.server.ServerTickRateManager`
- Vanilla source: `net.minecraft.server.dedicated.ServerWatchdog`
- [Bevy ECS system scheduling](https://bevyengine.org/learn/book/getting-started/ecs/)
- [Tokio interval documentation](https://docs.rs/tokio/latest/tokio/time/fn.interval.html)
- [Game Programming Patterns — Game Loop](https://gameprogrammingpatterns.com/game-loop.html)
