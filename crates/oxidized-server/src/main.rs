//! Oxidized — a high-performance Minecraft Java Edition server.
//!
//! This is the binary entry point. It configures the global allocator,
//! parses CLI arguments, initialises tracing, loads configuration, and
//! launches the Tokio async runtime.

mod cli;
mod config;
mod logging;

use clap::Parser;
use mimalloc::MiMalloc;
use oxidized_protocol::constants;
use tracing::info;

use crate::cli::Args;
use crate::config::ServerConfig;

/// Use mimalloc as the global allocator for improved throughput and
/// reduced fragmentation under the server's allocation patterns.
/// See [ADR-029](../../docs/adr/adr-029-memory-management.md).
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialise structured logging before anything else.
    logging::init(&args.log_level);

    info!(
        "Starting Oxidized v{} (Minecraft {})",
        env!("CARGO_PKG_VERSION"),
        constants::GAME_VERSION,
    );
    info!(
        "Protocol version: {} | World version: {}",
        constants::PROTOCOL_VERSION,
        constants::WORLD_VERSION,
    );

    // Load (or create) server.properties.
    let mut config = ServerConfig::load_or_create(&args.config)?;

    // CLI overrides take precedence over server.properties.
    if let Some(port) = args.port {
        config.server_port = port;
    }
    if let Some(ref world) = args.world {
        config.level_name.clone_from(world);
    }

    // Validate configuration.
    config.validate().map_err(|e| anyhow::anyhow!("{e}"))?;

    // If --init-settings was passed, save defaults and exit.
    if args.init_settings {
        config.save(&args.config)?;
        info!("Initialized '{}' — exiting.", args.config.display());
        return Ok(());
    }

    info!(
        "Server port: {} | Online mode: {} | Max players: {}",
        config.server_port, config.online_mode, config.max_players,
    );
    info!(
        "World: '{}' | View distance: {} | Simulation distance: {}",
        config.level_name, config.view_distance, config.simulation_distance,
    );

    // Build the Tokio runtime for async networking and I/O.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("oxidized-worker")
        .build()?;

    runtime.block_on(async {
        // Register Ctrl+C / SIGTERM shutdown signal.
        let shutdown = tokio::signal::ctrl_c();

        info!("Server ready — press Ctrl+C to stop.");

        // Await shutdown signal.
        if let Err(e) = shutdown.await {
            tracing::error!("Failed to listen for shutdown signal: {e}");
        }

        info!("Server stopped.");
        Ok(())
    })
}
