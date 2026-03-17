# ADR-005: Configuration Management

| Field | Value |
|-------|-------|
| Status | **Superseded by [ADR-033](adr-033-configuration-format.md)** (format decision) |
| Date | 2026-03-17 |
| Phases | P01, P19 |
| Deciders | Oxidized Core Team |

> **Note:** The format decision in this ADR (Java `.properties`) has been superseded by
> [ADR-033: Configuration Format Evolution](adr-033-configuration-format.md), which adopts
> TOML as the sole configuration format. The remaining decisions (validation on load,
> environment variable overrides, CLI precedence, hot-reload architecture) remain in effect.

## Context

The vanilla Minecraft server uses a Java `.properties` file format for its primary configuration (`server.properties`). This format is a flat key-value store with string values, no nesting, no typed fields, and limited comments. Every value is parsed from a string at runtime — a misconfigured port as `"abc"` only fails when the server tries to bind. The format has been stable for over a decade, and every Minecraft server operator knows it. Thousands of hosting panels, tutorials, and scripts expect `server.properties` to exist and follow the vanilla format.

We face a tension: modern configuration formats (TOML, YAML) offer nesting, typed values, and better ergonomics, but migrating away from `server.properties` would break the expectations of the entire Minecraft server hosting ecosystem. Server operators switching from vanilla to Oxidized should be able to drop in their existing `server.properties` and have it work.

Beyond file format, we need a configuration architecture that supports validation on load (not at first use), environment variable overrides (for containerized deployments), CLI flag precedence (for quick testing), and eventually hot-reload for game rules that can change at runtime (Phase 19).

## Decision Drivers

- **Vanilla compatibility**: operators must be able to use their existing `server.properties` file without modification
- **Strong typing internally**: the parsed configuration must be a Rust struct with typed fields — no stringly-typed lookups at runtime
- **Validation on load**: all configuration errors should be reported at startup, not when the value is first accessed during gameplay
- **Container-friendly**: Docker/Kubernetes deployments must be able to override any setting via environment variables
- **Forward compatibility**: unknown keys in `server.properties` should be preserved (not discarded) so vanilla-generated files aren't silently truncated
- **Sensible defaults**: if no configuration file exists, generate one with commented defaults that explain each option

## Considered Options

### Option 1: Parse server.properties only (vanilla compatibility)

Read and write the vanilla `server.properties` format exclusively. Use a custom parser (the format is simple: `key=value` lines, `#` comments). This maximizes compatibility but limits us to flat key-value pairs — any Oxidized-specific settings that need nesting (e.g., per-world configurations) would require awkward conventions like `world.overworld.spawn-x=0`.

### Option 2: TOML as primary, server.properties adapter

Use TOML as the canonical configuration format (an `oxidized.toml` file). Provide a migration tool that reads `server.properties` and writes `oxidized.toml`. This gives us a modern format with nesting, typed values, and inline tables, but forces operators to learn a new format and breaks compatibility with existing hosting tools.

### Option 3: Both formats with auto-detection

Support both `server.properties` and `oxidized.toml`. On startup, check which file(s) exist and parse accordingly. If both exist, one takes precedence. This is flexible but introduces ambiguity — operators may not realize which file is being used, leading to confusing behavior when they edit one file but the other is loaded.

### Option 4: YAML

Use YAML for configuration. While YAML supports nesting and types, it has well-known pitfalls (the Norway problem, implicit type coercion, whitespace sensitivity) and is increasingly being replaced by TOML in the Rust ecosystem. YAML parsers are also heavier dependencies than `.properties` parsers.

## Decision

**We parse vanilla `server.properties` as the primary configuration format for maximum compatibility.** Internally, the parsed configuration is a strongly-typed Rust struct deserialized via a custom parser. Oxidized-specific settings that don't exist in vanilla are supported via `oxidized-` prefixed keys in the same file.

### Configuration Struct

```rust
#[derive(Debug, Clone)]
pub struct ServerConfig {
    // Vanilla settings
    pub server_port: u16,               // default: 25565
    pub max_players: u32,               // default: 20
    pub motd: String,                   // default: "An Oxidized Minecraft Server"
    pub online_mode: bool,              // default: true
    pub view_distance: u8,              // default: 10
    pub simulation_distance: u8,        // default: 10
    pub level_name: String,             // default: "world"
    pub level_seed: String,             // default: ""
    pub gamemode: GameMode,             // default: Survival
    pub difficulty: Difficulty,         // default: Easy
    pub spawn_protection: u32,          // default: 16
    pub enable_command_block: bool,     // default: false
    pub allow_flight: bool,             // default: false
    pub pvp: bool,                      // default: true
    pub enable_rcon: bool,              // default: false
    pub rcon_port: u16,                 // default: 25575
    pub rcon_password: String,          // default: ""
    pub enable_query: bool,             // default: false
    pub query_port: u16,               // default: 25565
    pub white_list: bool,              // default: false
    // ... all other vanilla keys

    // Oxidized-specific (prefixed in .properties)
    pub oxidized_log_format: LogFormat, // default: Pretty
    pub oxidized_tick_threads: usize,   // default: num_cpus
    // ... future Oxidized-specific settings
}
```

### Precedence Order

Configuration values are resolved in this order (highest priority first):

1. **CLI flags**: `--port 25566` overrides everything
2. **Environment variables**: `OXIDIZED_SERVER_PORT=25566` (prefix `OXIDIZED_`, dots become underscores, uppercase)
3. **server.properties file**: the on-disk configuration
4. **Compiled defaults**: hardcoded defaults in the `ServerConfig` struct

### Validation on Load

All values are validated immediately when the configuration is parsed:

```rust
impl ServerConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = parse_properties(path)?;
        let config = Self::from_raw(raw)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.server_port == 0 {
            return Err(ConfigError::InvalidValue {
                key: "server-port",
                value: "0",
                reason: "port must be between 1 and 65535",
            });
        }
        if self.view_distance == 0 || self.view_distance > 32 {
            return Err(ConfigError::InvalidValue {
                key: "view-distance",
                value: &self.view_distance.to_string(),
                reason: "must be between 1 and 32",
            });
        }
        // ... all validations
        Ok(())
    }
}
```

### Unknown Key Preservation

When parsing `server.properties`, unknown keys are stored in a `BTreeMap<String, String>` and written back when the file is saved. This ensures that if vanilla adds new keys in future versions, Oxidized doesn't silently discard them.

### Default File Generation

If no `server.properties` exists on first startup, Oxidized generates one with all default values and explanatory comments:

```properties
# Oxidized Minecraft Server Configuration
# See https://oxidized.rs/docs/configuration for details

# The port the server listens on
server-port=25565

# Maximum number of players
max-players=20

# Message displayed in the server list
motd=An Oxidized Minecraft Server
```

### Hot-Reload (Phase 19)

Game rules and select settings (motd, max-players, whitelist) will support runtime modification via the `/gamerule` command and RCON. The `ServerConfig` struct will be wrapped in an `Arc<ArcSwap<ServerConfig>>` to allow atomic swaps without locking the tick loop. File-level hot-reload (watching `server.properties` for changes) is deferred to a later phase.

## Consequences

### Positive

- Zero friction for operators migrating from vanilla — drop in existing `server.properties` and it works
- Strong typing catches configuration errors at startup, not during gameplay
- Environment variable overrides enable seamless Docker/Kubernetes deployment without volume-mounting config files
- Unknown key preservation prevents data loss when Oxidized doesn't recognize a vanilla key
- Default file generation with comments helps new operators understand available settings

### Negative

- The `.properties` format limits us to flat key-value pairs — complex Oxidized-specific settings need awkward key naming conventions
- Custom parser required — no off-the-shelf `.properties` crate handles the full vanilla format (Unicode escapes, multiline values, continuation lines)
- Maintaining parity with vanilla's ever-growing list of settings is an ongoing effort as new Minecraft versions add keys

### Neutral

- A future `oxidized.toml` file may be introduced alongside `server.properties` for Oxidized-specific advanced settings, without replacing the vanilla file
- The CLI flag parser uses `clap` which provides `--help` generation and shell completions for free

## Compliance

- **Integration test**: a test loads every vanilla `server.properties` key from the Minecraft wiki and verifies Oxidized parses it without error
- **Round-trip test**: write defaults → read back → compare — ensures serialization is lossless
- **Unknown key test**: parse a file with `future-key=value` and verify it appears in the unknown keys map and is written back
- **Environment override test**: set `OXIDIZED_SERVER_PORT=12345` and verify it overrides the file value
- **Code review**: any new `ServerConfig` field must have a corresponding validation check and default value

## Related ADRs

- [ADR-001: Async Runtime Selection](adr-001-async-runtime.md) — runtime configuration (thread count) is a config setting
- [ADR-004: Logging, Tracing & Observability](adr-004-logging-observability.md) — log format and level are configurable
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — server-port, max-players, and rate limits come from config

## References

- [Java .properties file format specification](https://docs.oracle.com/javase/8/docs/api/java/util/Properties.html)
- [Vanilla server.properties — Minecraft Wiki](https://minecraft.wiki/w/Server.properties)
- [clap — Rust CLI argument parser](https://docs.rs/clap/latest/clap/)
- [arc-swap — Lock-free atomic swaps](https://docs.rs/arc-swap/latest/arc_swap/)
