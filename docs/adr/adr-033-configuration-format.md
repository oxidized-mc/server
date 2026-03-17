# ADR-033: Configuration Format Evolution

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P01 (retrofit), P19 (hot-reload) |
| Supersedes | ADR-005 (format decision only) |
| Deciders | Oxidized Core Team |

## Context

ADR-005 chose `server.properties` (Java Properties format) for maximum vanilla compatibility. After implementation and retrospective, we identified fundamental limitations:

1. **No type safety** — everything is strings; `server-port=abc` only fails at runtime parse
2. **No nesting** — Oxidized-specific settings need awkward `oxidized-` prefixes
3. **No native comments** — the parser can read `#` comments but `save()` regenerates the file from scratch, losing hand-edited comments
4. **No arrays/lists** — pack lists, world configurations, etc. need comma-delimited strings
5. **Java-specific format** — we're building a Rust server; `.properties` is a Java convention with no Rust ecosystem support

The key insight: **the Minecraft client never reads `server.properties`**. It's purely server-side. Server operators interact with it, but they can learn a better format — especially one that's an improvement.

TOML is the Rust ecosystem's standard configuration format. It's used by Cargo, rustfmt, clippy, and hundreds of Rust projects. It offers typed values, hierarchical sections, inline comments, arrays, and tables — all of which directly benefit server operators.

## Decision Drivers

- **Rust-native**: TOML is the de facto Rust config format; serde_toml provides compile-time type safety
- **Human-readable**: TOML is designed to be read and written by humans
- **Hierarchical**: settings group naturally into `[network]`, `[gameplay]`, `[world]`, `[admin]`, etc.
- **Typed**: integers, booleans, strings, and arrays are first-class — no string parsing at runtime
- **Comment-preserving**: `toml_edit` crate preserves comments and formatting through read-modify-write cycles
- **Forward compatibility**: unknown keys preserved via `#[serde(flatten)]`

## Considered Options

### Option 1: Keep server.properties (status quo, ADR-005)

Keep the Java Properties format. Pros: zero migration friction. Cons: all the limitations listed above. Rejected because the limitations compound as Oxidized adds features vanilla doesn't have.

### Option 2: TOML as sole configuration format ✅

Use `oxidized.toml` as the only configuration format. This is a new Rust server, not a drop-in replacement for vanilla — there are no existing `server.properties` files to migrate from. Operators configure Oxidized from scratch using a modern, typed, hierarchical format.

### Option 3: YAML

Rejected. YAML has the Norway problem (`NO` → `false`), implicit type coercion, and whitespace sensitivity. It's being replaced by TOML across the Rust ecosystem.

### Option 4: JSON

Rejected. No comments, verbose syntax, not designed for human editing.

### Option 5: RON (Rust Object Notation)

Rejected. Niche format, unfamiliar to non-Rust developers, poor tooling support.

## Decision

**TOML (`oxidized.toml`) is the sole configuration format.** There is no `server.properties` support — Oxidized is a new server, not a vanilla wrapper. No migration tooling is needed.

### Configuration Structure

```toml
# Oxidized Minecraft Server Configuration
# See https://github.com/dodoflix/Oxidized/docs/configuration.md

[network]
port = 25565
ip = ""
online_mode = true
prevent_proxy_connections = false
compression_threshold = 256
use_native_transport = true
rate_limit = 0
accepts_transfers = false

[gameplay]
gamemode = "survival"
difficulty = "easy"
hardcore = false
force_gamemode = false
max_players = 20
spawn_protection = 16
max_world_size = 29999984
allow_flight = false
spawn_npcs = true
spawn_animals = true
spawn_monsters = true
allow_nether = true
max_chained_neighbor_updates = 1000000
pvp = true

[world]
name = "world"
seed = ""
generate_structures = true
view_distance = 10
simulation_distance = 10
sync_chunk_writes = true
region_file_compression = "deflate"

[display]
motd = "An Oxidized Minecraft Server"
enable_status = true
hide_online_players = false
entity_broadcast_range_percentage = 100
status_heartbeat_interval = 5

[admin]
white_list = false
enforce_whitelist = false
op_permission_level = 4
function_permission_level = 2
enforce_secure_profile = true
log_ips = true
max_tick_time = 60000
player_idle_timeout = 0
broadcast_console_to_ops = true
broadcast_rcon_to_ops = true
pause_when_empty_seconds = 60

[rcon]
enabled = false
port = 25575
password = ""

[query]
enabled = false
port = 25565

[resource_pack]
url = ""
sha1 = ""
prompt = ""
required = false

[management]
enabled = false
host = ""
port = 0
secret = ""
tls_enabled = false

[packs]
initial_enabled = "vanilla"
initial_disabled = ""

[advanced]
enable_jmx_monitoring = false
text_filtering_config = ""
text_filtering_version = 0
enable_code_of_conduct = false
bug_report_link = ""
```

### Serde Integration

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub gameplay: GameplayConfig,
    #[serde(default)]
    pub world: WorldConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub rcon: RconConfig,
    #[serde(default)]
    pub query: QueryConfig,
    #[serde(default)]
    pub resource_pack: ResourcePackConfig,
    #[serde(default)]
    pub management: ManagementConfig,
    #[serde(default)]
    pub packs: PacksConfig,
    #[serde(default)]
    pub advanced: AdvancedConfig,

    /// Unknown keys preserved for forward compatibility
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}
```

Each sub-struct (e.g., `NetworkConfig`) also derives `Serialize, Deserialize, Default` with typed fields and `#[serde(default)]` on each field.

### Startup Behavior

1. If `oxidized.toml` exists → load and validate it
2. If `oxidized.toml` doesn't exist → generate default with all sections and inline comments

### Validation

Same as ADR-005: all values validated immediately on load. `validate()` returns `Result<(), ConfigError>` with descriptive errors.

### Precedence Order (unchanged from ADR-005)

1. CLI flags (`--port 25566`)
2. Environment variables (`OXIDIZED_NETWORK_PORT=25566` — section + field, uppercase, underscores)
3. `oxidized.toml` file
4. Compiled defaults

### Comment Preservation

For hot-reload (Phase 19), use `toml_edit` to parse the document, modify values in-place, and write back — preserving all user comments and formatting.

## Consequences

### Positive

- Type-safe config: invalid values caught at deserialization, not at first use
- Hierarchical sections: settings are logically grouped
- serde derives: adding a new config field is one line of code + one line of default
- Comments survive: operators' notes are preserved through hot-reload
- Ecosystem alignment: consistent with Cargo, rustfmt, clippy, deny.toml

### Negative

- Operators familiar with vanilla `server.properties` need to learn TOML (minimal friction — TOML is intuitive)
- `toml` and `toml_edit` are additional dependencies (but well-maintained, widely used in Rust ecosystem)

### Neutral

- No `server.properties` support means no Java Properties parser to maintain
- If community demand arises for vanilla import, it can be added as an optional CLI tool later

## Compliance

- **Deserialization test**: load a TOML string with all fields set to non-default values, verify each typed field
- **Serialization test**: create a non-default config, serialize to TOML, verify sections and keys
- **Round-trip test**: serialize → deserialize → compare (all fields)
- **Default generation test**: default `oxidized.toml` is valid and contains all sections with comments
- **Unknown key test**: parse TOML with `[custom_section]`, verify it survives roundtrip via `extra`

## Related ADRs

- [ADR-005: Configuration Management](adr-005-configuration.md) — **Superseded** (format decision only)
- [ADR-004: Logging, Tracing & Observability](adr-004-logging-observability.md) — log format/level are configurable
- [ADR-006: Network I/O Architecture](adr-006-network-io.md) — port, max-players, rate limits come from config

## References

- [TOML specification](https://toml.io/)
- [toml crate (serde integration)](https://docs.rs/toml/latest/toml/)
- [toml_edit (comment-preserving)](https://docs.rs/toml_edit/latest/toml_edit/)

