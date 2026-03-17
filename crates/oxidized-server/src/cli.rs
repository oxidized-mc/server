//! Command-line argument parsing for the Oxidized server.
//!
//! Mirrors the vanilla Minecraft server's CLI options with Rust-idiomatic
//! naming and additional options for observability.

use std::path::PathBuf;

use clap::Parser;

/// Oxidized — a high-performance Minecraft Java Edition server written in Rust.
#[derive(Parser, Debug)]
#[command(name = "oxidized")]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Override the server port (overrides server.properties value).
    #[arg(long)]
    pub port: Option<u16>,

    /// Override the world/level name (overrides server.properties value).
    #[arg(long)]
    pub world: Option<String>,

    /// Set the universe directory (parent of world folders).
    #[arg(long, default_value = ".")]
    pub universe: PathBuf,

    /// Disable GUI (always headless — this flag exists for vanilla compatibility).
    #[arg(long)]
    pub nogui: bool,

    /// Set the minimum log level.
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Path to server.properties configuration file.
    #[arg(long, default_value = "server.properties")]
    pub config: PathBuf,

    /// Force upgrade world data on startup.
    #[arg(long)]
    pub force_upgrade: bool,

    /// Initialize server.properties and exit.
    #[arg(long)]
    pub init_settings: bool,

    /// Run in demo mode.
    #[arg(long)]
    pub demo: bool,

    /// Erase cached world data.
    #[arg(long)]
    pub erase_cache: bool,

    /// Load with vanilla datapack only (safe mode).
    #[arg(long)]
    pub safe_mode: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults() {
        let args = Args::try_parse_from(["oxidized"]).expect("default args should parse");
        assert_eq!(args.port, None);
        assert_eq!(args.world, None);
        assert_eq!(args.universe, PathBuf::from("."));
        assert!(!args.nogui);
        assert_eq!(args.log_level, "info");
        assert_eq!(args.config, PathBuf::from("server.properties"));
        assert!(!args.force_upgrade);
        assert!(!args.init_settings);
        assert!(!args.demo);
        assert!(!args.erase_cache);
        assert!(!args.safe_mode);
    }

    #[test]
    fn parse_all_flags() {
        let args = Args::try_parse_from([
            "oxidized",
            "--port",
            "25565",
            "--world",
            "survival",
            "--universe",
            "/data/worlds",
            "--nogui",
            "--log-level",
            "debug",
            "--config",
            "/etc/oxidized/server.properties",
            "--force-upgrade",
            "--init-settings",
            "--demo",
            "--erase-cache",
            "--safe-mode",
        ])
        .expect("full args should parse");

        assert_eq!(args.port, Some(25565));
        assert_eq!(args.world.as_deref(), Some("survival"));
        assert_eq!(args.universe, PathBuf::from("/data/worlds"));
        assert!(args.nogui);
        assert_eq!(args.log_level, "debug");
        assert_eq!(
            args.config,
            PathBuf::from("/etc/oxidized/server.properties")
        );
        assert!(args.force_upgrade);
        assert!(args.init_settings);
        assert!(args.demo);
        assert!(args.erase_cache);
        assert!(args.safe_mode);
    }

    #[test]
    fn reject_invalid_port() {
        let result = Args::try_parse_from(["oxidized", "--port", "not_a_number"]);
        assert!(result.is_err(), "non-numeric port should be rejected");
    }

    #[test]
    fn reject_unknown_flag() {
        let result = Args::try_parse_from(["oxidized", "--nonexistent"]);
        assert!(result.is_err(), "unknown flags should be rejected");
    }
}
