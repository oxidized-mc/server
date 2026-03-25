//! Shared protocol and game constants for Minecraft 26.1.
//!
//! All values sourced from `net.minecraft.SharedConstants` in the
//! decompiled vanilla server JAR.

// === Version Info ===

/// The human-readable game version string (e.g. `"26.1"`).
pub const GAME_VERSION: &str = "26.1";

/// The version name sent in the server status response.
///
/// This is what clients display in the server list (e.g., "26.1").
pub const VERSION_NAME: &str = GAME_VERSION;

/// Wire protocol version negotiated during the handshake.
pub const PROTOCOL_VERSION: i32 = 775;

/// Internal world/data version used for save-file migrations.
pub const WORLD_VERSION: i32 = 4786;

/// Version series identifier (`"main"` for mainline releases).
pub const SERIES: &str = "main";

/// Network protocol version for full releases.
pub const RELEASE_NETWORK_PROTOCOL_VERSION: i32 = 775;

/// Incremental snapshot protocol version layered on top of the release version.
pub const SNAPSHOT_NETWORK_PROTOCOL_VERSION: i32 = 303;

/// Whether this build is a development snapshot.
pub const SNAPSHOT: bool = false;

/// Major version of the client resource-pack format.
pub const RESOURCE_PACK_FORMAT_MAJOR: i32 = 84;

/// Minor version of the client resource-pack format.
pub const RESOURCE_PACK_FORMAT_MINOR: i32 = 0;

/// Major version of the server data-pack format.
pub const DATA_PACK_FORMAT_MAJOR: i32 = 101;

/// Minor version of the server data-pack format.
pub const DATA_PACK_FORMAT_MINOR: i32 = 1;

/// Realms/RPC management server API version.
pub const RPC_MANAGEMENT_SERVER_API_VERSION: &str = "2.0.0";

// === Networking ===

/// Default listening port for the Minecraft server.
pub const DEFAULT_PORT: u16 = 25565;

/// Byte threshold above which packets are zlib-compressed.
pub const DEFAULT_COMPRESSION_THRESHOLD: i32 = 256;

/// Seconds before an idle connection is considered timed-out.
pub const CONNECTION_TIMEOUT_SECS: u64 = 30;

// === Tick Timing ===

/// Server ticks executed per real-time second.
pub const TICKS_PER_SECOND: u32 = 20;

/// Milliseconds between consecutive ticks at full speed.
pub const MILLIS_PER_TICK: u64 = 50;

/// Ticks in one real-time minute (20 × 60).
pub const TICKS_PER_MINUTE: u32 = 1200;

/// Ticks in one full in-game day/night cycle.
pub const TICKS_PER_GAME_DAY: u32 = 24_000;

/// Default value for the `randomTickSpeed` game rule.
pub const DEFAULT_RANDOM_TICK_SPEED: u32 = 3;

/// Ticks between automatic world saves (5 real-time minutes).
pub const AUTOSAVE_INTERVAL_TICKS: u32 = 6000;

// === World Geometry ===

/// Width (and depth) of a chunk section in blocks.
pub const SECTION_WIDTH: usize = 16;

/// Height of a single chunk section in blocks.
pub const SECTION_HEIGHT: usize = 16;

/// Total number of blocks in one chunk section (16 × 16 × 16).
pub const SECTION_SIZE: usize = SECTION_WIDTH * SECTION_WIDTH * SECTION_HEIGHT;

/// Number of vertical sections in an overworld chunk (y −64 … 319).
pub const SECTION_COUNT: usize = 24;

/// Block-to-chunk coordinate shift (region resolution in blocks).
pub const WORLD_RESOLUTION: i32 = 16;

/// Maximum client render distance in chunks.
pub const MAX_RENDER_DISTANCE: i32 = 32;

/// Hard limit for the world border radius in blocks.
pub const MAX_WORLD_SIZE: i32 = 29_999_984;

// === Chat / Commands ===

/// Maximum length of a chat message in characters.
pub const MAX_CHAT_LENGTH: usize = 256;

/// Maximum length of a command entered by a player.
pub const MAX_USER_INPUT_COMMAND_LENGTH: usize = 32_500;

/// Maximum length of a command inside a function file.
pub const MAX_FUNCTION_COMMAND_LENGTH: usize = 2_000_000;

/// Maximum allowed length for a player display name.
pub const MAX_PLAYER_NAME_LENGTH: usize = 16;

/// Cap on recursive neighbor-update propagation to prevent infinite loops.
pub const MAX_CHAINED_NEIGHBOR_UPDATES: i32 = 1_000_000;

// === Player Defaults ===

/// Default maximum number of players allowed on the server.
pub const MAX_PLAYERS_DEFAULT: u32 = 20;

/// Default server view distance in chunks.
pub const DEFAULT_VIEW_DISTANCE: u32 = 10;

/// Default entity simulation distance in chunks.
pub const DEFAULT_SIMULATION_DISTANCE: u32 = 10;

/// Default spawn-protection radius in blocks.
pub const DEFAULT_SPAWN_PROTECTION: u32 = 16;

// === Server Defaults ===

/// Default Message-Of-The-Day shown in the server list.
pub const DEFAULT_MOTD: &str = "A Minecraft Server";

/// Default name for the primary world directory.
pub const DEFAULT_LEVEL_NAME: &str = "world";

/// Default RCON listening port.
pub const DEFAULT_RCON_PORT: u16 = 25575;

/// Maximum milliseconds a single tick may take before the watchdog fires.
pub const DEFAULT_MAX_TICK_TIME_MS: i64 = 60_000;

/// Seconds the server waits before pausing when no players are online.
pub const DEFAULT_PAUSE_WHEN_EMPTY_SECS: i32 = 60;

// === Game Time Presets ===

/// Day-time tick at dawn (`/time set day`).
pub const DAY_START_TICKS: i64 = 1000;

/// Day-time tick at noon (`/time set noon`).
pub const NOON_TICKS: i64 = 6000;

/// Day-time tick at dusk (`/time set night`).
pub const NIGHT_START_TICKS: i64 = 13000;

/// Day-time tick at midnight (`/time set midnight`).
pub const MIDNIGHT_TICKS: i64 = 18000;

// === Misc ===

/// NBT tag key that stores the data version inside saved structures.
pub const DATA_VERSION_TAG: &str = "DataVersion";

/// Width and height (in pixels) of the `server-icon.png`.
pub const WORLD_ICON_SIZE: i32 = 64;

/// Maximum blast resistance a block can have (bedrock-level).
pub const MAXIMUM_BLOCK_EXPLOSION_RESISTANCE: f32 = 3_600_000.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_size_equals_4096() {
        assert_eq!(SECTION_SIZE, 4096);
    }

    #[test]
    fn test_millis_per_tick_derived_from_ticks_per_second() {
        assert_eq!(MILLIS_PER_TICK, 1000 / TICKS_PER_SECOND as u64);
    }

    #[test]
    fn test_ticks_per_minute_derived_from_ticks_per_second() {
        assert_eq!(TICKS_PER_MINUTE, TICKS_PER_SECOND * 60);
    }

    #[test]
    fn test_protocol_version_is_release() {
        // Release protocol versions do NOT have the 30th bit set (0x40000000).
        let version = PROTOCOL_VERSION;
        assert_eq!(version & 0x4000_0000, 0);
        assert_eq!(version, RELEASE_NETWORK_PROTOCOL_VERSION);
    }

    #[test]
    fn test_default_port_equals_25565() {
        assert_eq!(DEFAULT_PORT, 25565);
    }
}
