# ADR-004: Logging, Tracing & Observability

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P01, P38 |
| Deciders | Oxidized Core Team |

## Context

The vanilla Minecraft server uses Log4j with basic string-formatted messages like `logger.info("Player {} joined from {}", name, address)`. While functional, this approach lacks structured data — log messages are free-form strings that are difficult to parse, filter, or correlate programmatically. There's no concept of "spans" (a request's lifecycle across multiple function calls), making it hard to trace a single player's connection from handshake through login to play. When investigating production issues, operators resort to grepping log files with regex patterns.

A modern game server needs more than printf-style logging. We need structured events with typed key-value fields, hierarchical spans that track request lifecycles, configurable output formats (human-readable for development, JSON for production log aggregation), and the ability to export telemetry data to external systems (Prometheus, Grafana, Jaeger) for monitoring at scale.

The vanilla server also suffers from excessive logging verbosity — during normal operation, hundreds of log lines per second are emitted for routine events, making it difficult to spot actual problems. We need a system where verbosity is configurable per-subsystem (e.g., `TRACE` for protocol debugging, `WARN` for world generation) and where hot paths can log at `trace!` level without any runtime cost when that level is disabled.

## Decision Drivers

- **Structured key-value data**: every log event should carry typed fields (player UUID, packet ID, chunk coordinates) not just formatted strings
- **Span-based request tracing**: a player's connection lifecycle should be a span tree — connection → state → packet handler → subsystem call
- **Zero-cost when disabled**: `trace!` and `debug!` events in hot paths (tick loop, packet processing) must compile to a no-op when the level is filtered out
- **Composable subscribers**: multiple output layers (stdout, file rotation, JSON, OpenTelemetry) should coexist without code changes
- **Per-subsystem filtering**: operators must be able to set `oxidized_protocol=debug,oxidized_world=warn` independently
- **Ecosystem integration**: Tokio and most async Rust libraries already instrument with `tracing` — we should get their spans for free

## Considered Options

### Option 1: log + env_logger

The `log` crate is Rust's original logging facade. It provides `info!`, `warn!`, `error!` macros and a simple `Log` trait. `env_logger` is the most common backend, configurable via the `RUST_LOG` environment variable. However, `log` only supports unstructured string messages — there are no spans, no key-value fields, and no way to correlate events across async task boundaries. It's adequate for simple CLI tools but insufficient for a concurrent server.

### Option 2: tracing + tracing-subscriber

The `tracing` crate extends the `log` model with structured fields and spans. Events carry typed key-value pairs: `info!(player = %uuid, "joined the server")`. Spans represent scoped operations: `let _span = info_span!("handle_packet", packet_id = id).entered()`. `tracing-subscriber` provides composable "layers" — `fmt::Layer` for human-readable output, `json::Layer` for structured output, `EnvFilter` for per-target filtering, and `opentelemetry` for export to distributed tracing backends. `tracing` is the standard for async Rust and is already used by Tokio, hyper, and tower.

### Option 3: slog

`slog` is a structured logging library predating `tracing`. It supports key-value pairs and hierarchical loggers (child loggers inherit parent fields). However, it has its own macro system incompatible with the `log` facade, limited async/span support, and a much smaller ecosystem. Most modern Rust libraries instrument with `tracing`, not `slog`, so we'd miss out on automatic instrumentation from our dependency tree.

## Decision

**We adopt the `tracing` crate as our instrumentation layer and `tracing-subscriber` as the output backend.** All logging in Oxidized uses `tracing` macros (`trace!`, `debug!`, `info!`, `warn!`, `error!`) with structured fields. Every significant operation is wrapped in a span.

### Span Hierarchy Design

```
server                                          (root span — server lifetime)
├── connection{addr="1.2.3.4:54321"}            (per-connection span)
│   ├── handshaking                             (protocol state span)
│   │   └── handle_packet{id=0x00}              (per-packet span)
│   ├── login{player="Steve"}                   (state transition adds player name)
│   │   ├── handle_packet{id=0x00}
│   │   ├── encryption_setup
│   │   └── compression_setup
│   ├── configuration                           (post-login configuration)
│   └── play{uuid="abc-def-..."}                (main gameplay — long-lived)
│       ├── handle_packet{id=0x14}
│       └── handle_packet{id=0x1A}
├── tick{number=12345}                          (per-tick span — 50ms cycle)
│   ├── entity_tick
│   ├── block_updates
│   └── chunk_io
└── world_gen{chunk="[4, -7]"}                  (background worldgen tasks)
```

### Subscriber Configuration

```rust
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,oxidized_protocol=debug")))
    .with(fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_span_events(fmt::format::FmtSpan::CLOSE))
    .init();
```

### Performance-Sensitive Hot Paths

Packet processing and tick loops use `trace!` level. When the filter is set to `info` (the default), these events are skipped at the callsite level — the `tracing` macro checks a cached `AtomicBool` and short-circuits before evaluating any arguments. This is effectively zero-cost.

```rust
// Hot path — zero cost when trace is disabled
trace!(packet_id = id, size = data.len(), "decoding packet");

// Normal operations — always visible at default level
info!(player = %uuid, world = %world_name, "player joined");

// Errors — always visible, carries structured context
error!(chunk_x = cx, chunk_z = cz, error = %e, "failed to load chunk");
```

### Future: OpenTelemetry & Metrics (Phase 38)

In Phase 38, we add `tracing-opentelemetry` as an additional subscriber layer. This exports spans and events to an OpenTelemetry collector (Jaeger, Zipkin, or OTLP endpoint). A Prometheus metrics endpoint (`/metrics`) exposes counters (packets processed, chunks loaded, ticks elapsed) and histograms (tick duration, packet decode time). These layers are added without changing any instrumentation code — only the subscriber setup changes.

## Consequences

### Positive

- Every log event carries structured, queryable fields — operators can filter by player UUID, packet type, or chunk coordinates
- Span hierarchy provides full request tracing from TCP accept to packet handling without manual correlation IDs
- Per-target filtering (`RUST_LOG=oxidized_world=trace`) enables surgical debugging of specific subsystems
- Zero-cost `trace!` in hot paths — no performance impact on production servers running at `info` level
- Automatic instrumentation from Tokio, hyper, and other `tracing`-aware dependencies

### Negative

- `tracing` spans in async code require `Instrument` combinators or `#[instrument]` attributes to propagate correctly across `.await` points — easy to forget
- The subscriber setup is more complex than `env_logger::init()` — multiple layers, filters, and formatters must be composed correctly

### Neutral

- Log output format differs from vanilla's Log4j format — operators familiar with vanilla logs will need to adjust (mitigated by similar timestamp/level/message structure)
- JSON structured output is opt-in via a configuration flag, defaulting to human-readable format

## Compliance

- **No `println!` or `eprintln!`**: CI lint denies `clippy::print_stdout` and `clippy::print_stderr` in non-test code — all output goes through `tracing` macros
- **Span on public async functions**: code review checks that public async functions in `oxidized-server` and `oxidized-game` use `#[instrument(skip_all)]` or manual span entry
- **Structured fields required**: code review rejects format-string-only events like `info!("player {name} at {x},{z}")` — must use `info!(player = %name, x = x, z = z, "player position")`
- **Hot path audit**: any event in a function called more than once per tick must use `trace!` level — `debug!` or higher in a per-tick function is flagged in review

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — Tokio instruments its internals with `tracing` spans
- [ADR-002: Error Handling Strategy](adr-002-error-handling.md) — errors are logged with `tracing` events carrying the error chain
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — connection tasks create per-connection spans
- [ADR-008: Connection State Machine](adr-008-connection-state-machine.md) — state transitions are logged as span transitions

## References

- [tracing crate documentation](https://docs.rs/tracing/latest/tracing/)
- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/)
- [Tokio tracing integration](https://tokio.rs/tokio/topics/tracing)
- [OpenTelemetry Rust SDK](https://docs.rs/opentelemetry/latest/opentelemetry/)
- [tracing best practices — Eliza Weisman](https://docs.rs/tracing/latest/tracing/#best-practices)
