//! Oxidized — a high-performance Minecraft Java Edition server.
//!
//! This is the binary entry point. It configures the global allocator,
//! parses CLI arguments, initialises tracing, loads configuration, and
//! launches the Tokio async runtime with the TCP listener.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clap::Parser;
use mimalloc::MiMalloc;
use oxidized_game::commands::Commands;
use oxidized_game::player::PlayerList;
use oxidized_game::worldgen::ChunkGenerator;
use oxidized_game::worldgen::flat::{FlatChunkGenerator, FlatWorldConfig};
use oxidized_nbt::NbtCompound;
use oxidized_protocol::chat::Component;
use oxidized_protocol::constants;
use oxidized_protocol::crypto::ServerKeyPair;
use oxidized_protocol::status::{ServerStatus, StatusPlayers, StatusVersion};
use oxidized_protocol::types::resource_location::ResourceLocation;
use oxidized_world::anvil::{AnvilChunkLoader, AsyncChunkLoader, ChunkSerializer};
use oxidized_world::registry::BlockRegistry;
use oxidized_world::storage::{LevelStorageSource, PrimaryLevelData};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use oxidized_server::app::cli::Args;
use oxidized_server::config::ServerConfig;
use oxidized_server::network::{
    LoginContext, NetworkContext, ServerContext, ServerSettings, WorldContext,
};
use oxidized_server::{app, network, tick};

/// Use mimalloc as the global allocator for improved throughput and
/// reduced fragmentation under the server's allocation patterns.
/// See [ADR-029](../../docs/adr/adr-029-memory-management.md).
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialise structured logging before anything else.
    app::logging::init(&args.log_level);

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

    // Environment variable overrides (ADR-005/033 precedence: CLI > env > file > defaults).
    config.apply_env_overrides();

    // CLI overrides take precedence over environment variables and oxidized.toml.
    if let Some(port) = args.port {
        config.network.port = port;
    }
    if let Some(ref world) = args.world {
        config.world.name.clone_from(world);
    }

    // Validate configuration.
    config.validate().map_err(|e| anyhow::anyhow!("{e}"))?;

    // If --init-settings was passed, save defaults and exit.
    if args.is_init_settings {
        config.save(&args.config)?;
        info!(path = %args.config.display(), "Initialized settings — exiting");
        return Ok(());
    }

    info!(
        port = config.network.port,
        is_online_mode = config.network.is_online_mode,
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
            is_secure_chat_enforced: false,
        });

        // Generate the RSA-1024 keypair for online-mode encryption.
        info!("Generating RSA-1024 keypair...");
        let keypair = ServerKeyPair::generate()
            .map_err(|e| anyhow::anyhow!("RSA key generation failed: {e}"))?;
        info!("RSA keypair generated");

        // Load world metadata from level.dat (or use defaults for new worlds).
        let storage = LevelStorageSource::new(&config.world.name);
        let level_dat_path = storage.level_dat_path();
        let mut level_data = if level_dat_path.exists() {
            match PrimaryLevelData::load(&level_dat_path) {
                Ok(data) => {
                    info!(world = %data.settings.level_name, "Loaded level.dat");
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

        let block_registry = Arc::new(
            BlockRegistry::load()
                .map_err(|e| anyhow::anyhow!("failed to load block registry: {e}"))?,
        );

        // Create the chunk generator (flat world for now).
        let chunk_generator: Arc<dyn ChunkGenerator> =
            Arc::new(FlatChunkGenerator::new(FlatWorldConfig::default()));

        // Create the chunk loader and serializer for disk I/O.
        let region_dir = storage.region_dir(oxidized_world::storage::Dimension::Overworld);
        let anvil_loader = AnvilChunkLoader::new(&region_dir, Arc::clone(&block_registry));
        let chunk_loader = Arc::new(AsyncChunkLoader::new(anvil_loader));
        let chunk_serializer = Arc::new(ChunkSerializer::new(Arc::clone(&block_registry)));

        // Initialize seed and spawn position for new worlds.
        let is_new_world = !level_dat_path.exists();
        if is_new_world {
            level_data.spawn.y = chunk_generator.find_spawn_y();

            // Derive world seed from config: parse as i64, hash string seeds,
            // or generate a random seed if empty.
            level_data.settings.world_seed = if config.world.seed.is_empty() {
                rand::random::<i64>()
            } else if let Ok(numeric) = config.world.seed.parse::<i64>() {
                numeric
            } else {
                // Hash non-numeric seed strings (same as vanilla).
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                config.world.seed.hash(&mut hasher);
                hasher.finish() as i64
            };

            info!(
                spawn_y = level_data.spawn.y,
                world_seed = level_data.settings.world_seed,
                "New world initialized"
            );

            // Persist immediately so seed and spawn position survive crashes
            // before the first autosave.
            if let Err(e) = level_data.save(&level_dat_path) {
                warn!(error = %e, "Failed to save initial level.dat");
            }
        }

        // Load per-player operator permissions from ops.json (same directory
        // as the config file, matching vanilla's server root convention).
        let ops_path = args
            .config
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("ops.json");
        let ops_store = Arc::new(oxidized_server::ops::OpsStore::load(
            &ops_path,
            config.admin.op_permission_level,
        ));

        let server_ctx = Arc::new(ServerContext {
            world: WorldContext {
                level_data: parking_lot::RwLock::new(level_data),
                dimensions,
                chunks: dashmap::DashMap::new(),
                dirty_chunks: dashmap::DashSet::new(),
                storage,
                block_registry,
                chunk_generator,
                chunk_loader,
                chunk_serializer,
                game_rules: parking_lot::RwLock::new(
                    oxidized_game::level::GameRules::default(),
                ),
            },
            network: NetworkContext {
                broadcast_tx: broadcast::channel(256).0,
                shutdown_tx: shutdown_tx.clone(),
                kick_channels: dashmap::DashMap::new(),
                player_list: parking_lot::RwLock::new(PlayerList::new(
                    config.gameplay.max_players as usize,
                )),
                max_players: config.gameplay.max_players as usize,
            },
            settings: ServerSettings {
                max_view_distance: config.world.view_distance as i32,
                max_simulation_distance: config.world.simulation_distance as i32,
                op_permission_level: config.admin.op_permission_level,
                spawn_protection: config.gameplay.spawn_protection,
                color_char,
                timeouts: config.network.timeouts.clone(),
                connection_rate_limit: config.network.connection_rate_limit.clone(),
                entity_tracking: config.gameplay.entity_tracking.clone(),
                weather: config.gameplay.weather.clone(),
                inbound_channel_capacity: config.advanced.inbound_channel_capacity,
                outbound_channel_capacity: config.advanced.outbound_channel_capacity,
                chunk_cache_size: config.world.chunk_cache_size,
                max_concurrent_chunk_generations: config.world.max_concurrent_chunk_generations,
            },
            commands: Commands::new(),
            event_bus: oxidized_game::event::EventBus::new(),
            tick_rate_manager: parking_lot::RwLock::new(
                oxidized_game::level::ServerTickRateManager::default(),
            ),
            ops: ops_store,
            self_ref: std::sync::OnceLock::new(),
        });
        server_ctx.init_self_ref();

        // Build the shared login context.
        let login_ctx = Arc::new(LoginContext {
            server_status,
            keypair: Arc::new(keypair),
            is_online_mode: config.network.is_online_mode,
            compression_threshold: config.network.compression_threshold,
            is_preventing_proxy_connections: config.network.is_preventing_proxy_connections,
            http_client: reqwest::Client::new(),
            server_ctx,
        });

        // --- Plugin initialization hook ---
        // Future plugin system will load and initialize plugins here,
        // after ServerContext is built but before the TCP listener starts.
        // Plugins will receive `Arc<ServerContext>` to:
        //   - Register event handlers via `server_ctx.event_bus`
        //   - Register custom commands via `server_ctx.commands.register()`
        //   - Read server configuration
        // This ordering guarantees all plugin hooks are in place before
        // the first client connection arrives.

        // Clone the server context for the console and shutdown save before moving login_ctx.
        let console_server_ctx = login_ctx.server_ctx.clone();
        let shutdown_server_ctx = login_ctx.server_ctx.clone();

        // Spawn the server tick loop on a dedicated OS thread (ADR-019).
        let tick_shutdown = Arc::new(AtomicBool::new(false));
        let tick_ctx = login_ctx.server_ctx.clone();
        let tick_shutdown_clone = tick_shutdown.clone();
        let tick_thread = std::thread::Builder::new()
            .name("tick".into())
            .spawn(move || {
                tick::run_tick_loop(&tick_ctx, &tick_shutdown_clone);
            })
            .map_err(|e| anyhow::anyhow!("failed to spawn tick thread: {e}"))?;

        // Spawn the TCP listener.
        let listener_shutdown = shutdown_tx.subscribe();
        let listener_handle = tokio::spawn(async move {
            if let Err(e) = network::listen(addr, login_ctx, listener_shutdown).await {
                error!(error = %e, "Listener failed");
            }
        });

        // Spawn the console command reader on a dedicated OS thread.
        // Rustyline blocks on stdin, so it cannot run in a Tokio task.
        let console_thread = std::thread::Builder::new()
            .name("console".into())
            .spawn(move || {
                app::console::run_console_loop(console_server_ctx);
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
        tick_shutdown.store(true, Ordering::Relaxed);
        let _ = shutdown_tx.send(());

        // Save level.dat on shutdown (ADR-030: graceful shutdown saves).
        // Uses the shared save helper which runs on a blocking thread (ADR-015).
        {
            info!("Saving level.dat...");
            match tick::save_level_dat(&shutdown_server_ctx).await {
                Ok(()) => info!("level.dat saved successfully"),
                Err(e) => error!(error = %e, "Failed to save level.dat on shutdown"),
            }
        }

        // Flush all dirty chunks to region files.
        {
            let dirty_count = shutdown_server_ctx.world.dirty_chunks.len();
            if dirty_count > 0 {
                info!(dirty_count, "Saving dirty chunks...");
                match tick::save_dirty_chunks(&shutdown_server_ctx).await {
                    Ok(saved) => info!(saved, "Chunks saved successfully"),
                    Err(e) => error!(error = %e, "Failed to save chunks on shutdown"),
                }
            }
        }

        // Save all online players' data to disk.
        {
            let playerdata_dir = shutdown_server_ctx.world.storage.player_data_dir();
            let players: Vec<_> = {
                let list = shutdown_server_ctx.network.player_list.read();
                list.iter().cloned().collect()
            };
            if !players.is_empty() {
                info!("Saving {} player(s)...", players.len());
                let dir = playerdata_dir.clone();
                let player_data: Vec<_> = players
                    .iter()
                    .map(|p| {
                        let p = p.read();
                        (p.uuid, p.save_to_nbt())
                    })
                    .collect();
                if let Err(e) = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                    std::fs::create_dir_all(&dir)?;
                    for (uuid, nbt) in &player_data {
                        let path = dir.join(format!("{uuid}.dat"));
                        oxidized_nbt::write_file(&path, nbt)?;
                    }
                    Ok(())
                })
                .await
                {
                    error!(error = %e, "Failed to save player data on shutdown");
                }
            }
        }

        // Wait for the listener task to finish, with a timeout to avoid
        // hanging indefinitely if connections don't close cleanly.
        const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
        match tokio::time::timeout(SHUTDOWN_TIMEOUT, listener_handle).await {
            Ok(result) => {
                let _ = result;
            },
            Err(_) => {
                warn!(
                    "Graceful shutdown timed out after {}s, forcing exit",
                    SHUTDOWN_TIMEOUT.as_secs()
                );
            },
        }

        // Join the tick thread so it finishes cleanly before exit.
        const TICK_JOIN_TIMEOUT: Duration = Duration::from_secs(5);
        let tick_join_start = std::time::Instant::now();
        loop {
            if tick_thread.is_finished() {
                if let Err(e) = tick_thread.join() {
                    warn!("Tick thread panicked: {e:?}");
                }
                break;
            }
            if tick_join_start.elapsed() > TICK_JOIN_TIMEOUT {
                warn!(
                    "Tick thread did not exit within {}s",
                    TICK_JOIN_TIMEOUT.as_secs()
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        // Join the console thread so it doesn't outlive the runtime.
        // The thread should exit once shutdown is broadcast and the
        // console loop detects it.
        const CONSOLE_JOIN_TIMEOUT: Duration = Duration::from_secs(3);
        let join_start = std::time::Instant::now();
        loop {
            if console_thread.is_finished() {
                if let Err(e) = console_thread.join() {
                    warn!("Console thread panicked: {e:?}");
                }
                break;
            }
            if join_start.elapsed() > CONSOLE_JOIN_TIMEOUT {
                warn!(
                    "Console thread did not exit within {}s",
                    CONSOLE_JOIN_TIMEOUT.as_secs()
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        // --- Plugin shutdown hook ---
        // Future plugin system will notify plugins of shutdown here,
        // after the listener has stopped but before the process exits.
        // Plugins can flush state, save data, and clean up resources.

        info!("Server stopped");
        Ok(())
    })
}
