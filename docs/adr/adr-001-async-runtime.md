# ADR-001: Async Runtime Selection

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P01, P02, P10, P19, P26 |
| Deciders | Oxidized Core Team |

## Context

The vanilla Minecraft Java server uses Netty for network I/O (an NIO-based event loop) and a single-threaded game loop that processes all world ticks sequentially. While Netty is mature, the vanilla architecture couples networking tightly to a monolithic tick loop, limiting concurrency and making it difficult to scale across CPU cores. Chunk generation, disk I/O, and network encoding all contend for time on the same thread.

Oxidized is a ground-up Rust reimplementation. Rust's async ecosystem offers zero-cost abstractions over OS-level I/O multiplexing (epoll/kqueue/io_uring), but requires choosing a runtime that drives `Future` polling. This is a foundational decision — the runtime underpins networking, file I/O, timers, and task scheduling for the entire server.

We need a runtime that can handle thousands of concurrent player connections, drive the tick loop at a precise 50ms interval, offload CPU-heavy work (world generation, lighting) to blocking threads, and integrate smoothly with the rest of the Rust async ecosystem (tower, hyper, tungstenite, etc.).

## Decision Drivers

- **Ecosystem breadth**: libraries for TCP, UDP, HTTP, WebSocket, channels, timers, and file I/O should be readily available and well-maintained
- **Work-stealing scheduler**: player connections have bursty, uneven workloads — a work-stealing executor prevents hot-thread problems
- **Blocking task support**: world generation and disk I/O are CPU/IO-bound and must not starve async tasks
- **Production maturity**: the runtime must be battle-tested at scale (millions of connections, years of hardening)
- **Precise timer resolution**: the tick loop requires a stable 50ms interval with minimal drift
- **Community and long-term maintenance**: active development, responsive issue triage, broad contributor base

## Considered Options

### Option 1: Tokio

Tokio is the de facto standard async runtime for Rust. It provides a multi-threaded work-stealing scheduler, a single-threaded mode, `spawn_blocking` for offloading CPU-heavy work, and a comprehensive I/O driver (TCP, UDP, Unix sockets, timers, signals). Its ecosystem includes tokio-tungstenite (WebSocket), reqwest (HTTP), tokio-rustls (TLS), and deep integration with tower for middleware. Used in production by Cloudflare, Discord, AWS (Firecracker), and many others. The trade-off is a larger dependency footprint and slightly more complex configuration compared to minimal runtimes.

### Option 2: async-std

async-std mirrors the Rust standard library's API surface with async equivalents. It uses a simpler global executor model and is easier to learn. However, its ecosystem is significantly smaller than Tokio's — many critical libraries (tungstenite, hyper, tower) either don't support async-std or require compatibility shims. Development activity has slowed considerably since 2022, raising long-term maintenance concerns.

### Option 3: smol

smol is a minimal async runtime (~1500 lines) that provides basic I/O and task spawning. Its philosophy is "bring your own" — it gives building blocks and lets you compose them. While elegant, this means we'd need to build or integrate our own work-stealing executor, blocking task pool, and timer infrastructure. The ecosystem is small and largely maintained by a single developer. Good for embedded or minimal use cases, but risky for a high-throughput game server.

### Option 4: No async — blocking threads with threadpool

Skip async entirely: use OS threads with a threadpool (rayon or custom). Each connection gets a dedicated thread, blocking directly on `read()`/`write()`. This avoids async complexity (pinning, Send/Sync constraints, colored functions) but scales poorly — a server with 1000 players would need 2000+ OS threads (read + write per connection), consuming significant memory for stacks and causing scheduler overhead. Vanilla gets away with this via Netty's event loop, but we'd be regressing further.

## Decision

**We adopt Tokio as the async runtime for Oxidized.** Specifically, we use the multi-threaded work-stealing scheduler (`#[tokio::main]` with default thread count = CPU cores).

Tokio satisfies every driver: its ecosystem is unmatched in Rust, providing direct integration with the libraries we need for protocol handling, HTTP (RCON/query), and TLS. The work-stealing scheduler naturally balances load across cores as player connections spike and idle. `tokio::task::spawn_blocking` gives us a clean escape hatch for CPU-bound world generation and synchronous disk I/O without starving the async reactor.

For the tick loop, we use `tokio::time::interval(Duration::from_millis(50))` which compensates for processing time drift. Network I/O uses `tokio::net::TcpListener` and `TcpStream` with `into_split()` for separate read/write halves. Inter-task communication uses `tokio::sync::mpsc` (bounded channels with backpressure), `tokio::sync::broadcast` (for server-wide events), and `tokio::sync::RwLock` where shared mutable state is unavoidable.

## Consequences

### Positive

- Access to the largest Rust async ecosystem — tokio-tungstenite, reqwest, tower, hyper, tokio-rustls all work out of the box
- Work-stealing scheduler automatically balances uneven player workloads across CPU cores
- `spawn_blocking` cleanly separates CPU-heavy worldgen from latency-sensitive network tasks
- `tokio::time::interval` provides drift-compensating tick timing without manual bookkeeping
- Extensive production hardening — bugs are found and fixed quickly given the massive user base

### Negative

- Adds ~30 transitive dependencies to the build, increasing compile times by 10-15 seconds on first build
- Async Rust has a steeper learning curve — contributors must understand `Pin`, `Send`/`Sync` bounds, and structured concurrency
- Tokio's multi-threaded runtime has slightly higher per-task overhead than a single-threaded executor for simple workloads

### Neutral

- Tokio's `#[tokio::test]` macro simplifies async unit testing but requires all test authors to be aware of the async context
- We commit to Tokio's cancellation semantics (drop = cancel), which differs from some other runtimes

## Compliance

- **Workspace dependency**: Tokio must be declared in `[workspace.dependencies]` in the root `Cargo.toml` with the features `full` or the specific feature set `rt-multi-thread, macros, net, io-util, time, sync, signal, fs`
- **No other runtimes**: CI lint checks that no crate depends on `async-std`, `smol`, or `actix-rt`
- **No blocking in async context**: Clippy lint `#[deny(clippy::future_not_send)]` on public APIs; code review must flag any `std::thread::sleep` or blocking `std::fs` calls inside async functions
- **spawn_blocking audit**: Any function calling `spawn_blocking` must document why in a code comment

## Related ADRs

- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — uses Tokio's TcpListener and task spawning model
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — connection tasks are Tokio spawned futures
- [ADR-009: Encryption & Compression Pipeline](adr-009-encryption-compression.md) — wraps Tokio's AsyncRead/AsyncWrite

## References

- [Tokio documentation](https://docs.rs/tokio/latest/tokio/)
- [Tokio tutorial — spawning](https://tokio.rs/tokio/tutorial/spawning)
- [Alice Ryhl — "Actors with Tokio"](https://ryhl.io/blog/actors-with-tokio/)
- [Tokio work-stealing scheduler internals](https://tokio.rs/blog/2019-10-scheduler)
- [Discord — "Why Discord is switching from Go to Rust"](https://discord.com/blog/why-discord-is-switching-from-go-to-rust)
