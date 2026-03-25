# Phase 1 — Project Bootstrap

**Status:** ✅ Complete  
**Crate:** `oxidized-server`  
**Reward:** The project compiles, runs, logs a startup message, reads config.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-001: Async Runtime](../adr/adr-001-async-runtime.md) — Tokio runtime selection and async patterns
- [ADR-002: Error Handling](../adr/adr-002-error-handling.md) — thiserror for crate errors, anyhow at binary boundary
- [ADR-003: Crate Architecture](../adr/adr-003-crate-architecture.md) — 6-crate workspace DAG and module boundaries
- [ADR-004: Logging & Observability](../adr/adr-004-logging-observability.md) — tracing with structured spans and metrics
- [ADR-005: Configuration](../adr/adr-005-configuration.md) — TOML config parsing and validation (superseded by ADR-033)
- [ADR-030: Shutdown & Crash Handling](../adr/adr-030-shutdown-crash.md) — multi-layer shutdown with watchdog and crash reports


## Goal

Create the foundational workspace structure: logging, configuration, shared constants,
and a stubbed binary that starts and cleanly shuts down.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Server entry point | `net.minecraft.server.Main` |
| Dedicated server | `net.minecraft.server.dedicated.DedicatedServer` |
| Server properties | `net.minecraft.server.dedicated.DedicatedServerProperties` |
| Shared constants | `net.minecraft.SharedConstants` |
| Detected version | `net.minecraft.DetectedVersion` |

---

## Tasks

### 1.1 — Workspace `Cargo.toml`
- [x] Six-crate workspace: `oxidized-server`, `oxidized-protocol`, `oxidized-nbt`,
      `oxidized-world`, `oxidized-game`, `oxidized-macros`
- [x] Shared `[workspace.dependencies]` for all external crates
- [x] `[workspace.lints]` for clippy rules
- [x] `[profile.release]` with `lto = "thin"`, `opt-level = 3`

### 1.2 — Logging Setup (`oxidized-server/src/logging.rs`)
```rust
pub fn init(level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}
```

### 1.3 — `oxidized.toml` Parser (`oxidized-server/src/config.rs`)

All keys from `DedicatedServerProperties.java`:

```rust
#[derive(Debug, serde::Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub server_port: u16,               // 25565
    #[serde(default)]
    pub server_ip: String,              // "" = all interfaces
    #[serde(default = "default_true")]
    pub online_mode: bool,
    #[serde(default = "default_motd")]
    pub motd: String,                   // "A Minecraft Server"
    #[serde(default = "default_max_players")]
    pub max_players: u32,               // 20
    #[serde(default = "default_view_distance")]
    pub view_distance: u32,             // 10
    #[serde(default = "default_sim_distance")]
    pub simulation_distance: u32,       // 10
    #[serde(default = "default_level_name")]
    pub level_name: String,             // "world"
    pub level_seed: Option<i64>,
    #[serde(default = "default_difficulty")]
    pub difficulty: Difficulty,
    #[serde(default = "default_gamemode")]
    pub gamemode: GameType,
    #[serde(default)]
    pub hardcore: bool,
    #[serde(default)]
    pub allow_nether: bool,
    #[serde(default = "default_true")]
    pub spawn_npcs: bool,
    #[serde(default = "default_true")]
    pub spawn_animals: bool,
    #[serde(default = "default_true")]
    pub spawn_monsters: bool,
    #[serde(default = "default_spawn_protection")]
    pub spawn_protection: u32,          // 16
    #[serde(default = "default_compression")]
    pub network_compression_threshold: i32, // 256
    #[serde(default = "default_true")]
    pub use_native_transport: bool,
    pub enable_rcon: bool,
    #[serde(default = "default_rcon_port")]
    pub rcon_port: u16,                 // 25575
    pub rcon_password: String,
    pub enable_query: bool,
    #[serde(default = "default_port")]
    pub query_port: u16,
    pub management_server_enabled: bool,
    #[serde(default = "default_localhost")]
    pub management_server_host: String,
    pub management_server_port: u16,
    pub management_server_secret: String,
    #[serde(default = "default_true")]
    pub management_server_tls_enabled: bool,
    #[serde(default = "default_max_tick_time")]
    pub max_tick_time: i64,             // 60000 ms
    pub white_list: bool,
    #[serde(default = "default_true")]
    pub enforce_whitelist: bool,
}
```

- Load from `oxidized.toml` using the `toml` crate + serde derives
- Generate default file if missing
- Validate values on load (e.g. port in range 1–65535)

### 1.4 — Shared Constants (`oxidized-protocol/src/constants.rs`)

```rust
pub const PROTOCOL_VERSION: i32     = 775;
pub const WORLD_VERSION: i32        = 4786;
pub const GAME_VERSION: &str        = "26.1";
pub const DEFAULT_PORT: u16         = 25565;
pub const TICKS_PER_SECOND: u32     = 20;
pub const TICK_DURATION_MS: u64     = 50;
pub const SECTION_HEIGHT: usize     = 16;
pub const SECTION_WIDTH: usize      = 16;
pub const SECTION_SIZE: usize       = 4096;
pub const SECTION_COUNT: usize      = 24;
pub const AUTOSAVE_INTERVAL: u32    = 6000;
pub const COMPRESSION_THRESHOLD: i32 = 256;
pub const KEEPALIVE_INTERVAL: u32   = 20;
pub const CONNECTION_TIMEOUT_SECS: u64 = 30;
pub const MAX_CHAT_LENGTH: usize    = 256;
pub const MAX_PLAYERS_DEFAULT: u32  = 20;
```

### 1.5 — CLI args (`main.rs`)

```
oxidized [OPTIONS]
  --port <PORT>           Override server-port
  --world <PATH>          Override level-name
  --universe <PATH>       Set world folder parent
  --nogui                 No GUI (always true for headless)
  --log-level <LEVEL>     trace/debug/info/warn/error [default: info]
  --config <PATH>         Path to oxidized.toml [default: ./oxidized.toml]
  --force-upgrade         Upgrade world data on startup
```

### 1.6 — Main startup sequence

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_logging(&args.log_level);
    
    info!("Starting Oxidized {} (MC {})", env!("CARGO_PKG_VERSION"), GAME_VERSION);
    info!("Protocol version: {}", PROTOCOL_VERSION);
    
    let config = ServerConfig::load_or_create(&args.config)?;
    
    // Ctrl+C / SIGTERM shutdown signal
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    
    // TODO: start server (Phase 2+)
    info!("Server stopped.");
    Ok(())
}
```

---

## Data Structures

```rust
// src/config.rs
pub struct ServerConfig { … }

// src/main.rs
pub struct Args { port, world, universe, log_level, config, force_upgrade }
```

---

## Tests

- `test_config_defaults()` — load empty properties, verify all defaults
- `test_config_parse()` — parse known key=value pairs
- `test_config_invalid_port()` — port 0 or 65536 should error

---

## Files Created

```
crates/oxidized-server/src/
├── main.rs
├── config.rs
└── logging.rs

crates/oxidized-protocol/src/
└── constants.rs
```

---

## Completion Notes

**Initial implementation:** 2026-03-17 — 26 tests, 6 crates, CI green  
**Lifecycle retrofit:** 2026-03-17 — ADR audit + test audit + fixes

### Actual Test Coverage (post-retrofit)

| Category | Before | After |
|----------|--------|-------|
| Total tests | 26 | 48 |
| Config key parsing coverage | 6/41 (15%) | 41/41 (100%) |
| Roundtrip field coverage | 8/56 (14%) | 56/56 (100%) |
| Boundary validation tests | 0 | 13 |
| Format edge case tests | 0 | 6 |

### ADR Compliance

| ADR | Status |
|-----|--------|
| ADR-001 (Async Runtime) | ✅ Compliant |
| ADR-002 (Error Handling) | ✅ Compliant |
| ADR-003 (Crate Architecture) | ✅ Compliant |
| ADR-004 (Logging) | ✅ Fixed — structured `key=value` fields |
| ADR-005 (Configuration) | ✅ Fixed — unknown key preservation added |
| ADR-030 (Shutdown) | ✅ Compliant for Phase 1 scope |

### Issues Fixed During Retrofit

1. **Structured logging** — format strings replaced with `key=value` fields (ADR-004)
2. **Unknown key preservation** — `BTreeMap<String, String>` stores unrecognized keys (ADR-005)
3. **Test naming** — 9 tests in cli.rs and constants.rs renamed to follow convention
