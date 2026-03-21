//! Oxidized — a high-performance Minecraft Java Edition server.
//!
//! This is the binary entry point. It configures the global allocator,
//! parses CLI arguments, initialises tracing, loads configuration, and
//! launches the Tokio async runtime with the TCP listener.

mod cli;
mod config;
mod console;
mod logging;
mod network;

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use mimalloc::MiMalloc;
use oxidized_game::commands::Commands;
use oxidized_game::player::PlayerList;
use oxidized_nbt::NbtCompound;
use oxidized_protocol::chat::Component;
use oxidized_protocol::constants;
use oxidized_protocol::crypto::ServerKeyPair;
use oxidized_protocol::status::{ServerStatus, StatusPlayers, StatusVersion};
use oxidized_protocol::types::resource_location::ResourceLocation;
use oxidized_world::storage::PrimaryLevelData;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::cli::Args;
use crate::config::ServerConfig;
use crate::network::{LoginContext, ServerContext};

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
        let color_char = config.chat.color_char();
        let motd_component = match color_char {
            Some(ch) => Component::from_legacy_with_char(&config.display.motd, ch),
            None => Component::from_legacy(&config.display.motd),
        };
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
            description: motd_component,
            favicon: None, // Favicon loading deferred to Phase 18
            enforces_secure_chat: false,
        });

        // Generate the RSA-1024 keypair for online-mode encryption.
        info!("Generating RSA-1024 keypair...");
        let keypair = ServerKeyPair::generate()
            .map_err(|e| anyhow::anyhow!("RSA key generation failed: {e}"))?;
        info!("RSA keypair generated");

        // Load world metadata from level.dat (or use defaults for new worlds).
        let level_dat_path = format!("{}/level.dat", config.world.name);
        let level_data = if std::path::Path::new(&level_dat_path).exists() {
            match PrimaryLevelData::load(std::path::Path::new(&level_dat_path)) {
                Ok(data) => {
                    info!(world = %data.level_name, "Loaded level.dat");
                    data
                },
                Err(e) => {
                    warn!(error = %e, "Failed to load level.dat — using defaults");
                    PrimaryLevelData::from_nbt(&NbtCompound::new())
                        .map_err(|e| anyhow::anyhow!("failed to create default level data: {e}"))?
                },
            }
        } else {
            info!("No level.dat found — using default world metadata");
            PrimaryLevelData::from_nbt(&NbtCompound::new())
                .map_err(|e| anyhow::anyhow!("failed to create default level data: {e}"))?
        };

        // Build the shared server context for PLAY-state operations.
        let dimensions = vec![
            ResourceLocation::from_string("minecraft:overworld")
                .map_err(|e| anyhow::anyhow!("invalid dimension resource location: {e}"))?,
            ResourceLocation::from_string("minecraft:the_nether")
                .map_err(|e| anyhow::anyhow!("invalid dimension resource location: {e}"))?,
            ResourceLocation::from_string("minecraft:the_end")
                .map_err(|e| anyhow::anyhow!("invalid dimension resource location: {e}"))?,
        ];

        let server_ctx = Arc::new(ServerContext {
            player_list: parking_lot::RwLock::new(PlayerList::new(
                config.gameplay.max_players as usize,
            )),
            level_data,
            dimensions,
            max_view_distance: config.world.view_distance as i32,
            max_simulation_distance: config.world.simulation_distance as i32,
            chat_tx: broadcast::channel(256).0,
            color_char,
            commands: Commands::new(),
            max_players: config.gameplay.max_players as usize,
            shutdown_tx: shutdown_tx.clone(),
        });

        // Build the shared login context.
        let login_ctx = Arc::new(LoginContext {
            server_status,
            keypair: Arc::new(keypair),
            online_mode: config.network.online_mode,
            compression_threshold: config.network.compression_threshold,
            prevent_proxy_connections: config.network.prevent_proxy_connections,
            http_client: reqwest::Client::new(),
            server_ctx,
        });

        // Clone the server context for the console before moving login_ctx.
        let console_server_ctx = login_ctx.server_ctx.clone();

        // Spawn the TCP listener.
        let listener_shutdown = shutdown_tx.subscribe();
        let listener_handle = tokio::spawn(async move {
            if let Err(e) = network::listen(addr, login_ctx, listener_shutdown).await {
                error!(error = %e, "Listener failed");
            }
        });

        // Spawn the console command reader on a dedicated OS thread.
        // Rustyline blocks on stdin, so it cannot run in a Tokio task.
        let _console_thread = std::thread::Builder::new()
            .name("console".into())
            .spawn(move || {
                console::run_console_loop(console_server_ctx);
            })
            .map_err(|e| anyhow::anyhow!("failed to spawn console thread: {e}"))?;

        // Wait for Ctrl+C or shutdown from /stop command, then broadcast shutdown.
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result {
                    error!(error = %e, "Failed to listen for shutdown signal");
                }
            },
            _ = shutdown_rx.recv() => {
                // Shutdown was triggered by /stop command from console or player.
            },
        }
        info!("Shutdown signal received");
        let _ = shutdown_tx.send(());

        // Wait for the listener task to finish.
        let _ = listener_handle.await;

        info!("Server stopped");
        Ok(())
    })
}
