# ADR-030: Graceful Shutdown & Crash Recovery

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P01, P20, P38 |
| Deciders | Oxidized Core Team |

## Context

Minecraft server reliability depends on three shutdown scenarios being handled correctly:
graceful shutdown (operator issues `/stop` or sends SIGTERM), crash (unrecoverable error like
a panic or OOM), and force kill (SIGKILL, power loss, kernel OOM killer). Each scenario has
different constraints: graceful shutdown can take seconds to save state; crash shutdown must
preserve as much data as possible in milliseconds; force kill provides zero opportunity to
save — recovery depends entirely on previously persisted state.

Vanilla Java Edition has a dedicated watchdog thread (`ServerWatchdog`) that monitors the main
server thread. If a tick exceeds `max-tick-time` (default 60 seconds), the watchdog dumps all
thread stacks to a crash report, attempts a forced save, and calls `System.exit(1)`. This
catches infinite loops and deadlocks in the tick pipeline. Vanilla also writes crash reports
to `crash-reports/` on unhandled exceptions, including a detailed system profile (Java version,
OS, CPU, loaded mods, memory stats, recent game output). Operators depend on these crash
reports for diagnosis — they are often the first thing asked for in support channels.

For a Rust server, panics replace Java exceptions as the primary crash vector. By default, a
panic in any thread unwinds and terminates just that thread — but for a server, a panic in the
tick pipeline or world save is unrecoverable and should trigger a full crash report + shutdown.
We must also handle signals correctly in a multi-threaded tokio + rayon environment: SIGTERM
must propagate through async tasks, thread pools, and I/O systems in the right order.
Additionally, Rust's lack of GC means there's no finalizer mechanism — our shutdown sequence
must explicitly drop resources in the correct order.

## Decision Drivers

- **Data safety**: Player progress and world state must survive graceful shutdown and be
  recoverable after crashes. Data loss should be limited to at most the current tick.
- **Operator experience**: Crash reports must be actionable — not a bare backtrace, but a
  structured report with context (loaded chunks, entity count, last tick timing, etc.).
- **Deterministic shutdown**: Graceful shutdown must complete in bounded time and leave all
  data in a consistent state. No "zombie" processes or orphaned lock files.
- **Watchdog reliability**: The watchdog must function even if the tick thread is deadlocked
  or the tokio runtime is overloaded. It must be on a separate OS thread with no dependencies
  on the server's normal operation.
- **Startup recovery**: After an unclean shutdown, the server must detect corrupted state and
  repair it automatically where possible, or refuse to start with a clear error if not.
- **Signal handling**: SIGTERM and SIGINT must trigger graceful shutdown. SIGHUP could
  trigger config reload. Multiple SIGINTs should escalate to forced shutdown.

## Considered Options

### Option 1: Signal Handler + Graceful Drain

Register signal handlers that set an `AtomicBool` flag. The tick loop checks the flag each
tick and initiates shutdown. Simple and works for graceful shutdown.

**Pros**: Minimal complexity, well-understood pattern, works with tokio's signal handling.

**Cons**: Only handles graceful shutdown. No watchdog for stuck ticks. No crash reporting.
If the tick thread is stuck, the signal flag is never checked.

### Option 2: Separate Watchdog Process

Run the watchdog as a separate OS process that monitors the server via a heartbeat mechanism
(e.g., updating a shared memory timestamp each tick). If the heartbeat stops, the watchdog
kills the server and generates a report.

**Pros**: Fully independent — even if the server is completely deadlocked or OOM, the watchdog
can act. Can restart the server automatically.

**Cons**: Significantly more complex (IPC, process management). Cross-platform complications.
The watchdog process itself can fail. Overkill for this project — a watchdog thread within the
process is sufficient for the stuck-tick case.

### Option 3: Process Supervisor (systemd)

Delegate restart and health monitoring to systemd (or equivalent). The server is a simple
service that exits with appropriate codes; systemd handles restart.

**Pros**: Leverages existing infrastructure. Operators already use systemd for Minecraft
servers. No custom watchdog code.

**Cons**: Only handles process-level restart, not stuck-tick detection. No crash reporting.
Doesn't help with graceful shutdown orchestration within the process. Complementary to (not a
replacement for) in-process shutdown handling.

### Option 4: Checkpoint-Based Recovery

Periodically snapshot the entire server state to disk. On crash, restore from the last
checkpoint. Effectively a game-level equivalent of database WAL.

**Pros**: Minimal data loss (bounded by checkpoint interval). Recovery is straightforward
(load snapshot). Could enable server migration (transfer checkpoint to new host).

**Cons**: Extremely expensive for a Minecraft server (100K chunks × 28KB = 2.8GB per
checkpoint). Vanilla doesn't do this — it does incremental saves (autosave cycles through
chunks). Snapshot consistency is hard (must be a point-in-time view while ticks continue).
Not practical for the general case, but the autosave mechanism provides a similar guarantee.

## Decision

**Multi-layer shutdown with in-process watchdog.** Four layers handle the spectrum from
graceful to catastrophic:

### Layer 1: Graceful Shutdown (SIGTERM / SIGINT / `/stop`)

The shutdown sequence is initiated by setting an `AtomicBool` flag, which is checked at the
top of each tick. The sequence executes in a deterministic order:

```
1.  Log: "Server shutting down..."
2.  Set SHUTDOWN flag (AtomicBool, Ordering::SeqCst)
3.  Stop accepting new TCP connections (drop the TcpListener)
4.  Broadcast disconnect message to all players:
    Component::translatable("multiplayer.disconnect.server_shutdown")
5.  For each connected player (in parallel):
    a. Send ClientboundDisconnectPacket
    b. Flush the player's write buffer
    c. Save player data to <world>/playerdata/<uuid>.dat
    d. Close the TCP connection
6.  Wait for all player save tasks to complete (10s timeout)
7.  Save all dirty chunks:
    a. Mark all chunks as "saving" (prevents concurrent modification)
    b. Serialize chunk NBT
    c. Write to region files (with write markers, see Layer 4)
    d. fsync region files
8.  Save level.dat:
    a. Write to level.dat_new
    b. fsync level.dat_new
    c. Rename level.dat → level.dat_old
    d. Rename level.dat_new → level.dat
9.  Save scoreboard, bans, whitelist, ops lists
10. Shutdown the tokio runtime:
    a. Signal all async tasks to stop (via CancellationToken)
    b. Await task completion with 10s timeout
    c. Force-cancel remaining tasks
11. Shutdown the rayon thread pool (drop, waits for in-flight jobs)
12. Drop the ECS world (entities and components)
13. Drop the chunk map (frees chunk memory)
14. Drop registries and caches
15. Log: "Server stopped."
16. Flush log file, close log handles
17. Exit with code 0
```

**Multiple SIGINT escalation**: First SIGINT/SIGTERM → graceful shutdown. If a second signal
arrives while shutdown is in progress, escalate to forced shutdown: skip remaining save
operations, log a warning, exit with code 1. This handles cases where the graceful shutdown
itself is stuck.

**`/stop` command**: Sets the same `AtomicBool` flag. The tick loop detects it on the next
tick boundary and initiates the shutdown sequence. This guarantees the current tick completes
before shutdown begins — no mid-tick interruption.

### Layer 2: Watchdog Thread

A dedicated OS thread (not a tokio task, not a rayon job) monitors tick duration:

```rust
pub struct Watchdog {
    last_tick_start: Arc<AtomicU64>,  // millis since epoch
    max_tick_time_ms: u64,            // default: 60_000 (60s)
    shutdown_flag: Arc<AtomicBool>,
}

impl Watchdog {
    pub fn run(&self) {
        loop {
            std::thread::sleep(Duration::from_secs(5));

            if self.shutdown_flag.load(Ordering::Relaxed) {
                return;  // server is shutting down normally
            }

            let last_start = self.last_tick_start.load(Ordering::Relaxed);
            let elapsed = now_millis() - last_start;

            if elapsed > self.max_tick_time_ms {
                self.trigger_watchdog_crash(elapsed);
                return;
            }

            if elapsed > self.max_tick_time_ms / 2 {
                warn!("Server tick has been running for {elapsed}ms (threshold: {}ms)",
                      self.max_tick_time_ms);
            }
        }
    }
}
```

When the watchdog triggers:

1. Log: `"Server tick exceeded max-tick-time ({elapsed}ms > {max}ms)"`
2. Capture stack traces of all threads (`backtrace` crate for Rust threads).
3. Generate crash report (see format below).
4. Attempt to save critical data:
   a. Player data for all online players.
   b. Dirty chunks that are already serialized (don't block on serialization).
5. Write crash report to `crash-reports/crash-<timestamp>.txt`.
6. Log crash report path.
7. Exit with code 1.

The watchdog thread is spawned with `std::thread::Builder::new().name("watchdog")` before
the tokio runtime starts, ensuring it has no dependency on the async runtime.

### Layer 3: Panic Hook & Crash Report

A custom panic hook is installed at startup via `std::panic::set_hook`:

```rust
std::panic::set_hook(Box::new(|panic_info| {
    let report = CrashReport::from_panic(panic_info);
    report.write_to_file();
    report.log_summary();
    // Exit after panic — a panicked server is not recoverable
    std::process::exit(1);
}));
```

### Crash Report Format

```
---- Oxidized Server Crash Report ----
Time: 2026-03-17 14:23:45 UTC
Description: Server tick exceeded max-tick-time (65432ms > 60000ms)

-- System Details --
  Oxidized Version: 0.1.0 (build abc123)
  Operating System: Linux 6.8.0 (Ubuntu 24.04)
  Architecture: x86_64
  CPU: AMD Ryzen 9 7950X (16 cores)
  Total Memory: 32768 MB
  Process RSS: 4231 MB
  Process Uptime: 3d 14h 22m

-- Server State --
  Server TPS: 18.4 (last 1m), 19.8 (last 5m), 20.0 (last 15m)
  Current Tick: 5_832_410
  Tick Duration (last): 65432ms (EXCEEDED THRESHOLD)
  Online Players: 87 / 100
  Loaded Chunks: 94,312
  Loaded Entities: 48,201
  Pending Block Ticks: 12,843
  Pending Fluid Ticks: 3,291

-- Thread Dump --
  Thread "tick-main" (id=1):
    #0: oxidized::world::tick::process_block_entities (world/tick.rs:342)
    #1: oxidized::world::tick::tick_world (world/tick.rs:128)
    #2: oxidized::server::tick (server.rs:95)
    ...
  Thread "rayon-0" (id=5):
    #0: <idle>
  Thread "tokio-0" (id=8):
    #0: oxidized::net::flush_packets (net/flush.rs:67)
    ...

-- Recent Log Output --
  [14:22:40] [INFO] Player Steve moved to chunk (42, -17)
  [14:22:41] [WARN] Block entity tick at (672, 64, -271) taking unusually long
  [14:23:00] [WARN] Server tick has been running for 30000ms (threshold: 60000ms)
  ...

-- Loaded Worlds --
  overworld: chunks=62,410, entities=31,204
  the_nether: chunks=18,902, entities=9,847
  the_end: chunks=13,000, entities=7,150

-- Active Data Packs --
  vanilla (built-in)
  custom_loot (file/datapacks/custom_loot)
```

### Layer 4: Startup Recovery

On startup, before the server begins accepting connections, perform recovery checks:

**Region file integrity**:
- Each region file sector write is bracketed by a write marker: a magic byte sequence written
  before the sector data and a different magic byte after. On startup, scan region file headers
  for sectors where the "before" marker is present but the "after" marker is missing — these
  are interrupted writes.
- For corrupted sectors: regenerate the chunk from the region file's last known good state.
  If the chunk cannot be recovered, log a warning and mark it for regeneration (world gen will
  recreate it when a player enters range).

**level.dat recovery**:
- Try loading `level.dat`. If it's corrupted (invalid NBT, unexpected EOF), fall back to
  `level.dat_old`. If both are corrupted, refuse to start with a clear error message.
- The double-write pattern (write to `_new` → rename `→ _old` → rename `_new → level.dat`)
  ensures at least one valid copy exists at all times.

**Player data recovery**:
- Player data files (`playerdata/<uuid>.dat`) are written atomically (write to `.dat_new`,
  rename). On startup, if a `.dat_new` file exists alongside a `.dat`, the rename was
  interrupted — complete it. If only `.dat_new` exists, rename it. If the `.dat` file is
  corrupted, log a warning — the player will start with default state.

**Lock file**:
- On startup, create `<world>/session.lock` containing the process PID and start timestamp.
- If `session.lock` already exists, check if the PID is still running. If so, refuse to start
  (another server is using this world). If not, log a warning ("server did not shut down
  cleanly") and proceed with recovery.
- On graceful shutdown, delete `session.lock`.

## Consequences

### Positive

- **Comprehensive data safety**: The four-layer approach handles every shutdown scenario.
  Graceful shutdown saves everything. Watchdog saves what it can. Panic hook produces
  diagnostics. Startup recovery handles the rest.
- **Actionable crash reports**: The crash report format includes everything an operator needs
  to diagnose the issue — system state, thread dumps, recent logs, and server metrics. This
  is a significant improvement over a bare stack trace.
- **Bounded shutdown time**: Graceful shutdown has explicit timeouts at each stage. A stuck
  save won't prevent shutdown — after the timeout, the server proceeds to the next stage.
- **No zombie processes**: Multiple-SIGINT escalation and the watchdog thread ensure the
  server eventually exits, even if the normal shutdown path is stuck.
- **Automatic recovery**: Region file repair and level.dat fallback mean most unclean
  shutdowns are recovered automatically without operator intervention.

### Negative

- **Complexity**: Four layers of shutdown handling is significant code. Each layer must be
  tested independently, including crash-path code (which is inherently hard to test).
- **Watchdog false positives**: If `max-tick-time` is set too low, legitimate long ticks
  (e.g., world gen for many chunks) could trigger the watchdog. The default of 60s is
  conservative, but operators may need to tune it.
- **Incomplete crash saves**: The watchdog's "save what we can" attempt may produce
  inconsistent state (some chunks saved, some not). This is acceptable — it's better than
  losing everything — but operators should know to check for issues after a watchdog crash.
- **Panic hook limitations**: `std::process::exit(1)` in the panic hook skips destructors.
  Any data not already flushed to disk is lost. This is intentional — after a panic, we
  cannot trust the server's state enough to run normal shutdown.

### Neutral

- The crash report format is inspired by vanilla's but adapted for Rust (thread dumps use
  Rust backtrace format, no JVM-specific info). Operators familiar with vanilla crash
  reports will recognize the structure.
- systemd integration is complementary — operators should still use `Restart=on-failure` in
  the systemd unit file. Our in-process handling provides the crash report and data safety;
  systemd provides the process restart.

## Compliance

- [ ] SIGTERM triggers graceful shutdown completing in < 30s with all data saved.
- [ ] Second SIGINT/SIGTERM during shutdown escalates to forced exit within 5s.
- [ ] Watchdog detects a stuck tick (simulated infinite loop) within `max-tick-time + 10s`.
- [ ] Crash report is written to `crash-reports/` on panic, watchdog trigger, and OOM.
- [ ] Crash report contains: timestamp, description, system info, server state, thread dump.
- [ ] level.dat survives unclean shutdown (level.dat_old fallback works).
- [ ] Region file with interrupted write is detected and repaired on startup.
- [ ] session.lock prevents two servers from using the same world directory.
- [ ] Player data atomic write: kill during save → data recoverable on restart.
- [ ] `/stop` command completes shutdown within 30s for a server with 100K chunks.

## Related ADRs

- **ADR-001**: Server Bootstrap & Lifecycle (defines startup sequence, shutdown is the mirror)
- **ADR-002**: Async Runtime & Threading (tokio/rayon shutdown coordination)
- **ADR-006**: Chunk Storage & Management (region file write protocol)
- **ADR-029**: Memory Management & Allocation (shutdown drop order)
- **ADR-032**: Performance & Scalability Architecture (watchdog relates to tick timing)

## References

- [Vanilla `ServerWatchdog` (decompiled)](https://github.com/misode/mcmeta)
- [Vanilla crash report format](https://minecraft.wiki/w/Crash_report)
- [`std::panic::set_hook` documentation](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
- [`tokio::signal` for Unix signal handling](https://docs.rs/tokio/latest/tokio/signal/index.html)
- [backtrace-rs for stack trace capture](https://github.com/rust-lang/backtrace-rs)
- [systemd service hardening](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
