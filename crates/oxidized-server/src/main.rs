//! Oxidized — a high-performance Minecraft Java Edition server.
//!
//! This is the binary entry point. It configures the global allocator,
//! parses CLI arguments, initialises tracing, loads configuration, and
//! launches the Tokio async runtime with the TCP listener.

mod cli;
mod config;
mod logging;
mod network;

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use mimalloc::MiMalloc;
use oxidized_protocol::constants;
use oxidized_protocol::status::{Component, ServerStatus, StatusPlayers, StatusVersion};
use tokio::sync::broadcast;
use tracing::{error, info};

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
        version = env!("CARGO_PKG_VERSION"),
        minecraft = constants::GAME_VERSION,
        "Starting Oxidized",
    );
    info!(
        protocol = constants::PROTOCOL_VERSION,
        world_version = constants::WORLD_VERSION,
        "Protocol info",
    );

    // Load (or create) oxidized.toml.
    let mut config = ServerConfig::load_or_create(&args.config)?;

    // CLI overrides take precedence over oxidized.toml.
    if let Some(port) = args.port {
        config.network.port = port;
    }
    if let Some(ref world) = args.world {
        config.world.name.clone_from(world);
    }

    // Validate configuration.
    config.validate().map_err(|e| anyhow::anyhow!("{e}"))?;

    // If --init-settings was passed, save defaults and exit.
    if args.init_settings {
        config.save(&args.config)?;
        info!(path = %args.config.display(), "Initialized settings — exiting");
        return Ok(());
    }

    info!(
        port = config.network.port,
        online_mode = config.network.online_mode,
        max_players = config.gameplay.max_players,
        "Server configuration",
    );
    info!(
        world = %config.world.name,
        view_distance = config.world.view_distance,
        simulation_distance = config.world.simulation_distance,
        "World configuration",
    );

    // Build the Tokio runtime for async networking and I/O.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("oxidized-worker")
        .build()?;

    runtime.block_on(async {
        // Shutdown broadcast channel — all tasks listen for this signal.
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Resolve the bind address from config.
        let ip = if config.network.ip.is_empty() {
            "0.0.0.0".to_string()
        } else {
            config.network.ip.clone()
        };
        let addr: SocketAddr = format!("{ip}:{}", config.network.port)
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid bind address: {e}"))?;

        // Build the ServerStatus from config for the server list.
        let server_status = Arc::new(ServerStatus {
            version: StatusVersion {
                name: constants::VERSION_NAME.to_string(),
                protocol: constants::PROTOCOL_VERSION,
            },
            players: StatusPlayers {
                max: config.gameplay.max_players,
                online: 0,
                sample: Vec::new(),
            },
            description: Component::text(&config.display.motd),
            favicon: None, // Favicon loading deferred to Phase 18
            enforces_secure_chat: false,
        });

        // Spawn the TCP listener.
        let listener_shutdown = shutdown_tx.subscribe();
        let status_clone = Arc::clone(&server_status);
        let listener_handle = tokio::spawn(async move {
            if let Err(e) = network::listen(addr, status_clone, listener_shutdown).await {
                error!(error = %e, "Listener failed");
            }
        });

        // Wait for Ctrl+C, then broadcast shutdown.
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!(error = %e, "Failed to listen for shutdown signal");
        }
        info!("Shutdown signal received");
        let _ = shutdown_tx.send(());

        // Wait for the listener task to finish.
        let _ = listener_handle.await;

        info!("Server stopped");
        Ok(())
    })
}
