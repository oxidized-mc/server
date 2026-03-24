//! TCP listener and per-connection handler for the Oxidized server.
//!
//! Binds to the configured address, accepts incoming connections, and
//! spawns a Tokio task per client. Dispatches through the protocol
//! state machine: HANDSHAKING → STATUS/LOGIN → CONFIGURATION → PLAY.

mod configuration;
mod handshake;
pub mod helpers;
mod login;
mod play;
mod status;
pub mod writer;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::{DashMap, DashSet};
use oxidized_game::commands::Commands;
use oxidized_game::commands::source::ServerHandle;
use oxidized_game::event::EventBus;
use oxidized_game::level::game_rules::GameRules;
use oxidized_game::level::tick_rate::ServerTickRateManager;
use oxidized_game::level::weather::WeatherType;
use oxidized_game::worldgen::ChunkGenerator;
use oxidized_protocol::chat::Component;
use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState};
use oxidized_protocol::crypto::ServerKeyPair;
use oxidized_protocol::status::ServerStatus;
use oxidized_protocol::types::resource_location::ResourceLocation;
use oxidized_world::chunk::{ChunkPos, LevelChunk};
use oxidized_world::registry::BlockRegistry;
use oxidized_world::storage::{LevelStorageSource, PrimaryLevelData};
use parking_lot::RwLock;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use oxidized_game::player::PlayerList;

/// Shared state for login operations, passed to each connection handler.
pub struct LoginContext {
    /// Pre-built server status for the multiplayer list.
    pub server_status: Arc<ServerStatus>,
    /// RSA-1024 keypair used for encryption handshake.
    pub keypair: Arc<ServerKeyPair>,
    /// Whether the server authenticates players against Mojang session servers.
    pub is_online_mode: bool,
    /// Minimum packet size (in bytes) before compression is applied. `-1` disables compression.
    pub compression_threshold: i32,
    /// Whether to block connections through proxies by verifying the client IP.
    pub is_preventing_proxy_connections: bool,
    /// Reusable HTTP client for Mojang session server requests.
    pub http_client: reqwest::Client,
    /// Shared game server state for PLAY-state operations.
    pub server_ctx: Arc<ServerContext>,
}

/// Shared game server state accessible to all connection handlers.
///
/// Holds the player list, world metadata, and dimension registry needed
/// when a client transitions from CONFIGURATION to PLAY state.
pub struct ServerContext {
    /// Server-wide player roster (thread-safe via interior mutability).
    pub player_list: RwLock<PlayerList>,
    /// World metadata loaded from `level.dat` (mutable via tick loop).
    pub level_data: RwLock<PrimaryLevelData>,
    /// All registered dimension identifiers (e.g., `minecraft:overworld`).
    pub dimensions: Vec<ResourceLocation>,
    /// Maximum view distance allowed by the server config (2–32 chunks).
    pub max_view_distance: i32,
    /// Maximum simulation distance allowed by the server config (2–32 chunks).
    pub max_simulation_distance: i32,
    /// Broadcast channel for packets sent to all connected players.
    pub broadcast_tx: broadcast::Sender<BroadcastMessage>,
    /// Alternate color code prefix character, or `None` if disabled.
    pub color_char: Option<char>,
    /// Brigadier command framework — shared across all connections.
    pub commands: Commands,
    /// Game event bus for plugin extensibility.
    pub event_bus: EventBus,
    /// Maximum number of players allowed on the server.
    pub max_players: usize,
    /// Broadcast sender used to trigger a graceful server shutdown.
    pub shutdown_tx: broadcast::Sender<()>,
    /// Game rules — thread-safe for tick loop + command access.
    pub game_rules: RwLock<GameRules>,
    /// Tick rate manager — controls freeze/step/sprint.
    pub tick_rate_manager: RwLock<ServerTickRateManager>,
    /// World storage source — resolves paths to level.dat, region dirs, etc.
    pub storage: LevelStorageSource,
    /// Loaded chunk columns keyed by position. Thread-safe via `DashMap`.
    pub chunks: DashMap<ChunkPos, Arc<RwLock<LevelChunk>>>,
    /// Chunk positions that have been modified and need saving.
    pub dirty_chunks: DashSet<ChunkPos>,
    /// Block registry — loaded once at startup, shared across all handlers.
    pub block_registry: Arc<BlockRegistry>,
    /// Chunk generator — produces new chunks on demand for unseen positions.
    pub chunk_generator: Arc<dyn ChunkGenerator>,
    /// Operator permission level for all players (from server config).
    /// TODO: Replace with per-player ops from `ops.json`.
    pub op_permission_level: i32,
    /// Spawn protection radius (Chebyshev distance from world spawn).
    /// A value of 0 disables spawn protection.
    pub spawn_protection: u32,
    /// Per-player kick channels: signals a player's play loop to exit.
    pub kick_channels: DashMap<uuid::Uuid, tokio::sync::mpsc::Sender<String>>,
}

impl ServerContext {
    /// Sends a broadcast message to all connected players, logging a warning
    /// if no receivers are active.
    pub fn broadcast(&self, msg: BroadcastMessage) {
        if let Err(e) = self.broadcast_tx.send(msg) {
            warn!("Broadcast send failed: {e}");
        }
    }
}

impl ServerHandle for ServerContext {
    fn broadcast_to_ops(&self, _message: &Component, _min_level: u32) {
        // TODO(phase-18): broadcast to ops via broadcast_tx
    }

    fn request_shutdown(&self) {
        info!("Server shutdown requested via /stop");
        let _ = self.shutdown_tx.send(());
    }

    fn seed(&self) -> i64 {
        0 // TODO: expose world seed from PrimaryLevelData
    }

    fn online_player_names(&self) -> Vec<String> {
        self.player_list
            .read()
            .iter()
            .map(|p| p.read().name.clone())
            .collect()
    }

    fn online_player_count(&self) -> usize {
        self.player_list.read().player_count()
    }

    fn max_players(&self) -> usize {
        self.max_players
    }

    fn difficulty(&self) -> i32 {
        self.level_data.read().difficulty
    }

    fn game_time(&self) -> i64 {
        self.level_data.read().time
    }

    fn day_time(&self) -> i64 {
        self.level_data.read().day_time
    }

    fn is_raining(&self) -> bool {
        self.level_data.read().is_raining
    }

    fn is_thundering(&self) -> bool {
        self.level_data.read().is_thundering
    }

    fn kick_player(&self, name: &str, reason: &str) -> bool {
        // Find the player's UUID by name, then send a kick signal.
        let uuid = match self.find_player_uuid(name) {
            Some(u) => u,
            None => return false,
        };
        if let Some(tx) = self.kick_channels.get(&uuid) {
            let _ = tx.try_send(reason.to_string());
            true
        } else {
            false
        }
    }

    fn find_player_uuid(&self, name: &str) -> Option<uuid::Uuid> {
        let player_list = self.player_list.read();
        for player_arc in player_list.iter() {
            let player = player_arc.read();
            if player.name == name {
                return Some(player.uuid);
            }
        }
        None
    }

    fn command_descriptions(&self) -> Vec<(String, Option<String>)> {
        let mut cmds: Vec<(String, Option<String>)> = self
            .commands
            .dispatcher()
            .root
            .children
            .iter()
            .map(|(name, node)| (name.clone(), node.description().map(String::from)))
            .collect();
        cmds.sort_by(|a, b| a.0.cmp(&b.0));
        cmds
    }

    fn event_bus(&self) -> Option<&EventBus> {
        Some(&self.event_bus)
    }

    fn broadcast_chat(&self, message: &Component) {
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::ClientboundSystemChatPacket;

        let pkt = ClientboundSystemChatPacket {
            content: message.clone(),
            is_overlay: false,
        };
        let encoded = pkt.encode();
        let broadcast = BroadcastMessage {
            packet_id: ClientboundSystemChatPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        };
        self.broadcast(broadcast);
    }

    fn set_day_time(&self, time: i64) {
        self.level_data.write().day_time = time;
    }

    fn add_day_time(&self, ticks: i64) {
        let mut ld = self.level_data.write();
        ld.day_time = ld.day_time.wrapping_add(ticks);
    }

    fn set_weather(&self, weather: WeatherType, duration: Option<i32>) {
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::{ClientboundGameEventPacket, GameEventType};

        let was_raining;
        {
            let mut ld = self.level_data.write();
            was_raining = ld.is_raining;
            let dur = duration.unwrap_or(6000);
            match weather {
                WeatherType::Clear => {
                    ld.clear_weather_time = dur;
                    ld.is_raining = false;
                    ld.is_thundering = false;
                    ld.rain_time = 0;
                    ld.thunder_time = 0;
                },
                WeatherType::Rain => {
                    ld.clear_weather_time = 0;
                    ld.is_raining = true;
                    ld.is_thundering = false;
                    ld.rain_time = dur;
                    ld.thunder_time = dur;
                },
                WeatherType::Thunder => {
                    ld.clear_weather_time = 0;
                    ld.is_raining = true;
                    ld.is_thundering = true;
                    ld.rain_time = dur;
                    ld.thunder_time = dur;
                },
            }
        }

        // Broadcast weather change to all connected clients.
        let now_raining = self.level_data.read().is_raining;
        if was_raining != now_raining {
            let event = if now_raining {
                GameEventType::StartRaining
            } else {
                GameEventType::StopRaining
            };
            let pkt = ClientboundGameEventPacket { event, param: 0.0 };
            let encoded = pkt.encode();
            self.broadcast(BroadcastMessage {
                packet_id: ClientboundGameEventPacket::PACKET_ID,
                data: encoded.freeze(),
                exclude_entity: None,
                target_entity: None,
            });
        }
    }

    fn get_game_rule(&self, name: &str) -> Option<String> {
        let key = GameRules::from_name(name)?;
        Some(self.game_rules.read().get_as_string(key))
    }

    fn set_game_rule(&self, name: &str, value: &str) -> Result<(), String> {
        let key = GameRules::from_name(name).ok_or_else(|| format!("Unknown game rule: {name}"))?;
        self.game_rules.write().set_from_string(key, value)
    }

    fn game_rule_names(&self) -> Vec<&'static str> {
        GameRules::all_names()
    }

    fn tick_rate(&self) -> f32 {
        self.tick_rate_manager.read().tick_rate
    }

    fn set_tick_rate(&self, rate: f32) -> bool {
        self.tick_rate_manager.write().set_rate(rate)
    }

    fn is_tick_frozen(&self) -> bool {
        self.tick_rate_manager.read().is_frozen
    }

    fn set_tick_frozen(&self, is_frozen: bool) {
        self.tick_rate_manager.write().is_frozen = is_frozen;
    }

    fn tick_step(&self, steps: u32) {
        self.tick_rate_manager.write().request_steps(steps);
    }

    fn tick_steps_remaining(&self) -> u32 {
        self.tick_rate_manager.read().steps_remaining
    }

    fn tick_sprint(&self, ticks: u64) {
        self.tick_rate_manager.write().start_sprint(ticks);
    }

    fn is_tick_sprinting(&self) -> bool {
        self.tick_rate_manager.read().is_sprinting
    }

    fn broadcast_tick_state(&self) {
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::ClientboundTickingStatePacket;

        let mgr = self.tick_rate_manager.read();
        let pkt = ClientboundTickingStatePacket {
            tick_rate: mgr.tick_rate,
            is_frozen: mgr.is_frozen,
        };
        drop(mgr);
        let encoded = pkt.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundTickingStatePacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        });
    }

    fn set_player_game_mode(
        &self,
        uuid: uuid::Uuid,
        mode: oxidized_game::player::game_mode::GameMode,
    ) -> bool {
        use oxidized_game::player::abilities::PlayerAbilities;
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::{
            ClientboundGameEventPacket, ClientboundPlayerAbilitiesPacket,
            ClientboundPlayerInfoUpdatePacket, GameEventType, PlayerInfoActions, PlayerInfoEntry,
        };

        // Clone the Arc so we can drop the player_list lock early.
        let player_arc = {
            let player_list = self.player_list.read();
            match player_list.get(&uuid) {
                Some(p) => Arc::clone(p),
                None => return false,
            }
        };

        let entity_id;
        {
            let mut player = player_arc.write();
            if player.game_mode == mode {
                return false;
            }
            player.previous_game_mode = Some(player.game_mode);
            player.game_mode = mode;
            player.abilities = PlayerAbilities::for_game_mode(mode);
            entity_id = player.entity_id;
        }

        // Send ChangeGameMode event to the target player only.
        let game_event = ClientboundGameEventPacket {
            event: GameEventType::ChangeGameMode,
            param: mode.id() as f32,
        };
        let encoded = game_event.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundGameEventPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: Some(entity_id),
        });

        // Send updated abilities to the target player only.
        let abilities = player_arc.read().abilities.clone();
        let abilities_pkt = ClientboundPlayerAbilitiesPacket {
            flags: abilities.flags_byte(),
            fly_speed: abilities.fly_speed,
            walk_speed: abilities.walk_speed,
        };
        let encoded = abilities_pkt.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundPlayerAbilitiesPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: Some(entity_id),
        });

        // Broadcast tab list game mode update to all players.
        let player_name = player_arc.read().name.clone();
        let info_update = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(PlayerInfoActions::UPDATE_GAME_MODE),
            entries: vec![PlayerInfoEntry {
                uuid,
                name: player_name,
                properties: vec![],
                game_mode: mode.id(),
                latency: 0,
                is_listed: true,
                has_display_name: false,
                display_name: None,
                is_hat_visible: false,
                list_order: 0,
            }],
        };
        let encoded = info_update.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundPlayerInfoUpdatePacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        });

        true
    }

    fn send_system_message_to_player(&self, uuid: uuid::Uuid, message: &Component) {
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::ClientboundSystemChatPacket;

        let entity_id = {
            let player_list = self.player_list.read();
            match player_list.get(&uuid) {
                Some(p) => p.read().entity_id,
                None => return,
            }
        };

        let pkt = ClientboundSystemChatPacket {
            content: message.clone(),
            is_overlay: false,
        };
        let encoded = pkt.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundSystemChatPacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: Some(entity_id),
        });
    }

    fn set_block(&self, x: i32, y: i32, z: i32, block_name: &str) -> bool {
        use oxidized_protocol::codec::Packet;
        use oxidized_protocol::packets::play::ClientboundBlockUpdatePacket;
        use oxidized_protocol::types::BlockPos;

        let state_id = match self.block_registry.default_state(block_name) {
            Some(id) => u32::from(id.0),
            None => return false,
        };

        let pos = BlockPos::new(x, y, z);
        let chunk_pos = ChunkPos::from_block_coords(x, z);
        if let Some(chunk_ref) = self.chunks.get(&chunk_pos) {
            let mut chunk = chunk_ref.write();
            if chunk.set_block_state(x, y, z, state_id).is_ok() {
                self.dirty_chunks.insert(chunk_pos);
            } else {
                return false;
            }
        } else {
            return false;
        }

        // Broadcast block change to all players.
        let pkt = ClientboundBlockUpdatePacket {
            pos,
            block_state: state_id as i32,
        };
        let encoded = pkt.encode();
        self.broadcast(BroadcastMessage {
            packet_id: ClientboundBlockUpdatePacket::PACKET_ID,
            data: encoded.freeze(),
            exclude_entity: None,
            target_entity: None,
        });

        true
    }

    fn get_block(&self, x: i32, y: i32, z: i32) -> Option<String> {
        let chunk_pos = ChunkPos::from_block_coords(x, z);
        let chunk_ref = self.chunks.get(&chunk_pos)?;
        let chunk = chunk_ref.read();
        let state_id = chunk.get_block_state(x, y, z).ok()?;
        self.block_registry
            .block_name_from_state_id(state_id)
            .map(String::from)
    }
}

/// A packet broadcast to all connected players (or a targeted subset).
///
/// Used for chat, block updates, weather changes, tick state, and any other
/// packet that needs to reach clients.
#[derive(Debug, Clone)]
pub struct BroadcastMessage {
    /// Pre-encoded packet bytes (packet ID + body) ready for `send_raw`.
    pub packet_id: i32,
    /// Encoded packet body bytes.
    pub data: bytes::Bytes,
    /// If set, skip sending to the player with this entity ID.
    /// Used for block updates where the acting player already received an ack.
    pub exclude_entity: Option<i32>,
    /// If set, send ONLY to the player with this entity ID.
    /// Used for targeted packets like game mode changes and abilities updates.
    pub target_entity: Option<i32>,
}

/// Maximum valid serverbound PLAY packet ID for protocol 26.1-pre-3.
/// There are 69 registered serverbound packets (IDs 0x00–0x44).
const MAX_SERVERBOUND_PLAY_ID: i32 = 0x44;

/// Default maximum connections per IP within the rate-limiting window.
const DEFAULT_MAX_CONNECTIONS_PER_WINDOW: u32 = 10;

/// Default rate-limiting window duration.
const DEFAULT_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(10);

/// How often to clean up stale rate limiter entries.
const RATE_LIMIT_CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Simple per-IP connection rate limiter.
///
/// Tracks connection attempts within a sliding time window and rejects
/// excess connections. Stale entries are cleaned up periodically.
struct ConnectionRateLimiter {
    attempts: DashMap<IpAddr, (u32, Instant)>,
    max_per_window: u32,
    window: Duration,
}

impl ConnectionRateLimiter {
    fn new(max_per_window: u32, window: Duration) -> Self {
        Self {
            attempts: DashMap::new(),
            max_per_window,
            window,
        }
    }

    /// Returns `true` if the connection is allowed, `false` if rate-limited.
    fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.attempts.entry(ip).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

        if now.duration_since(*window_start) > self.window {
            *count = 1;
            *window_start = now;
            true
        } else if *count < self.max_per_window {
            *count += 1;
            true
        } else {
            false
        }
    }

    /// Removes entries older than the time window.
    fn cleanup(&self) {
        let now = Instant::now();
        self.attempts
            .retain(|_, (_, window_start)| now.duration_since(*window_start) <= self.window);
    }
}

/// Starts the TCP listener and accepts connections until a shutdown signal
/// is received.
///
/// # Errors
///
/// Returns an error if the listener fails to bind to `addr`.
pub async fn listen(
    addr: SocketAddr,
    ctx: Arc<LoginContext>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let rate_limiter = ConnectionRateLimiter::new(
        DEFAULT_MAX_CONNECTIONS_PER_WINDOW,
        DEFAULT_RATE_LIMIT_WINDOW,
    );
    let mut last_cleanup = Instant::now();
    info!(address = %addr, "Listening for connections");

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received — stopping listener");
                break;
            }

            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        // Periodic cleanup of stale rate limiter entries.
                        if last_cleanup.elapsed() > RATE_LIMIT_CLEANUP_INTERVAL {
                            rate_limiter.cleanup();
                            last_cleanup = Instant::now();
                        }

                        if !rate_limiter.check(peer_addr.ip()) {
                            warn!(
                                peer = %peer_addr,
                                "Connection rate limited — rejecting"
                            );
                            drop(stream);
                            continue;
                        }

                        info!(peer = %peer_addr, "New connection");
                        let ctx = Arc::clone(&ctx);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, peer_addr, &ctx).await {
                debug!(peer = %peer_addr, error = %e, "Connection handler finished with error");
                            }
                        });
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to accept connection");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles a single client connection through the protocol state machine.
///
/// Dispatches packets based on the current [`ConnectionState`]:
/// - **Handshaking** → parse [`ClientIntentionPacket`](oxidized_protocol::packets::handshake::ClientIntentionPacket), transition state
/// - **Status** → respond with server status JSON and pong
/// - **Login** → authenticate, enable encryption/compression, finish login
async fn handle_connection(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    ctx: &LoginContext,
) -> Result<(), ConnectionError> {
    let mut conn = Connection::new(stream, addr)?;
    let mut has_requested_status = false;
    debug!(
        peer = %addr,
        state = %conn.state,
        "Connection established",
    );

    loop {
        match conn.read_raw_packet().await {
            Ok(pkt) => {
                debug!(
                    peer = %addr,
                    state = %conn.state,
                    packet_id = format_args!("0x{:02X}", pkt.id),
                    size = pkt.data.len(),
                    "Received packet",
                );

                match conn.state {
                    ConnectionState::Handshaking => {
                        handshake::handle_handshake(&mut conn, pkt).await?;
                    },
                    ConnectionState::Status => {
                        let done =
                            status::handle_status(&mut conn, pkt, ctx, &mut has_requested_status)
                                .await?;
                        if done {
                            debug!(peer = %addr, "Status exchange complete");
                            return Ok(());
                        }
                    },
                    ConnectionState::Login => {
                        let profile = login::handle_login(&mut conn, pkt, ctx).await?;
                        let client_info = configuration::handle_configuration(&mut conn).await?;

                        // Vanilla: if a player with the same UUID is already online,
                        // kick the OLD connection and let the new one proceed.
                        let uuid = profile.uuid().unwrap_or_default();
                        if ctx.server_ctx.player_list.read().contains(&uuid) {
                            info!(
                                peer = %addr,
                                %uuid,
                                "Duplicate login — kicking old session",
                            );
                            // Send kick signal to old player's play loop.
                            if let Some(tx) = ctx.server_ctx.kick_channels.get(&uuid) {
                                let _ =
                                    tx.try_send("You logged in from another location".to_string());
                            }
                            // Give the old session time to disconnect cleanly.
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }

                        play::handle_play_entry(&mut conn, profile, client_info, ctx).await?;
                        info!(peer = %addr, "Player session ended");
                        return Ok(());
                    },
                    ConnectionState::Configuration | ConnectionState::Play => {
                        debug!(peer = %addr, state = %conn.state, "Unhandled state");
                        return Ok(());
                    },
                }
            },
            Err(ConnectionError::Io(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!(peer = %addr, state = %conn.state, "Client disconnected (EOF)");
                return Ok(());
            },
            Err(e) => {
                debug!(peer = %addr, state = %conn.state, error = %e, "Connection error");
                return Err(e);
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use oxidized_protocol::codec::Packet;
    use oxidized_protocol::codec::{frame, varint};
    use oxidized_protocol::constants;
    use oxidized_protocol::crypto::ServerKeyPair;
    use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
    use oxidized_protocol::packets::status::{
        ClientboundPongResponsePacket, ClientboundStatusResponsePacket,
    };
    use oxidized_protocol::packets::status::{
        ServerboundPingRequestPacket, ServerboundStatusRequestPacket,
    };
    use oxidized_protocol::status::{Component, StatusPlayers, StatusVersion};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    fn test_server_status() -> ServerStatus {
        ServerStatus {
            version: StatusVersion {
                name: constants::VERSION_NAME.to_string(),
                protocol: constants::PROTOCOL_VERSION,
            },
            players: StatusPlayers {
                max: 20,
                online: 0,
                sample: Vec::new(),
            },
            description: Component::text("Test Server"),
            favicon: None,
            is_secure_chat_enforced: false,
        }
    }

    fn test_login_context() -> Arc<LoginContext> {
        Arc::new(LoginContext {
            server_status: Arc::new(test_server_status()),
            keypair: Arc::new(ServerKeyPair::generate().unwrap()),
            is_online_mode: false,
            compression_threshold: -1,
            is_preventing_proxy_connections: false,
            http_client: reqwest::Client::new(),
            server_ctx: Arc::new(ServerContext {
                player_list: RwLock::new(PlayerList::new(20)),
                level_data: RwLock::new(
                    PrimaryLevelData::from_nbt(&oxidized_nbt::NbtCompound::new()).unwrap(),
                ),
                dimensions: vec![ResourceLocation::from_string("minecraft:overworld").unwrap()],
                max_view_distance: 10,
                max_simulation_distance: 10,
                broadcast_tx: broadcast::channel(256).0,
                color_char: Some('&'),
                commands: oxidized_game::commands::Commands::new(),
                event_bus: oxidized_game::event::EventBus::new(),
                max_players: 20,
                shutdown_tx: broadcast::channel(1).0,
                game_rules: RwLock::new(oxidized_game::level::GameRules::default()),
                tick_rate_manager: RwLock::new(
                    oxidized_game::level::ServerTickRateManager::default(),
                ),
                storage: LevelStorageSource::new(""),
                chunks: dashmap::DashMap::new(),
                dirty_chunks: dashmap::DashSet::new(),
                block_registry: Arc::new(BlockRegistry::load().unwrap()),
                chunk_generator: Arc::new(oxidized_game::worldgen::flat::FlatChunkGenerator::new(
                    oxidized_game::worldgen::flat::FlatWorldConfig::default(),
                )),
                op_permission_level: 4,
                spawn_protection: 16,
                kick_channels: dashmap::DashMap::new(),
            }),
        })
    }

    /// Sends a framed packet (VarInt length + VarInt packet_id + body) over a raw stream.
    async fn send_packet(stream: &mut TcpStream, packet_id: i32, body: &[u8]) {
        let mut inner = BytesMut::new();
        varint::write_varint_buf(packet_id, &mut inner);
        inner.extend_from_slice(body);
        frame::write_frame(stream, &inner).await.unwrap();
        stream.flush().await.unwrap();
    }

    /// Reads one framed packet and returns (packet_id, body).
    async fn read_packet(stream: &mut TcpStream) -> (i32, bytes::Bytes) {
        let frame_data =
            frame::read_frame(stream, oxidized_protocol::codec::frame::MAX_PACKET_SIZE)
                .await
                .unwrap();
        let mut buf = frame_data;
        let id = varint::read_varint_buf(&mut buf).unwrap();
        (id, buf)
    }

    #[tokio::test]
    async fn test_full_status_exchange() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Bind to a random port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let ctx_clone = Arc::clone(&ctx);
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // 1. Send handshake (intent = Status)
        let handshake = ClientIntentionPacket {
            protocol_version: constants::PROTOCOL_VERSION,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // 2. Send status request (empty body)
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // 3. Read status response
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        // Parse the response JSON string
        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);
        assert_eq!(json["description"]["text"], "Test Server");
        assert_eq!(json["players"]["max"], 20);

        // 4. Send ping request
        let ping_time: i64 = 1_719_000_000_000;
        let mut ping_body = BytesMut::new();
        oxidized_protocol::codec::types::write_i64(&mut ping_body, ping_time);
        send_packet(
            &mut client,
            ServerboundPingRequestPacket::PACKET_ID,
            &ping_body,
        )
        .await;

        // 5. Read pong response
        let (pong_id, pong_body) = read_packet(&mut client).await;
        assert_eq!(pong_id, ClientboundPongResponsePacket::PACKET_ID);
        let mut pong_data = pong_body;
        let echoed_time = types::read_i64(&mut pong_data).unwrap();
        assert_eq!(echoed_time, ping_time);

        // 6. Server should have closed our connection after pong
        // Reading should return EOF
        let mut eof_buf = [0u8; 1];
        let read_result = client.read(&mut eof_buf).await.unwrap();
        assert_eq!(read_result, 0, "expected EOF after pong");

        // Clean up
        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_protocol_mismatch_still_responds() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let ctx_clone = Arc::clone(&ctx);
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx_clone, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(bound_addr).await.unwrap();

        // Send handshake with WRONG protocol version
        let handshake = ClientIntentionPacket {
            protocol_version: 999,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: ClientIntent::Status,
        };
        let handshake_body = handshake.encode();
        send_packet(
            &mut client,
            ClientIntentionPacket::PACKET_ID,
            &handshake_body,
        )
        .await;

        // Send status request
        send_packet(&mut client, ServerboundStatusRequestPacket::PACKET_ID, &[]).await;

        // Should still get a valid response (vanilla behavior)
        let (resp_id, resp_body) = read_packet(&mut client).await;
        assert_eq!(resp_id, ClientboundStatusResponsePacket::PACKET_ID);

        use oxidized_protocol::codec::types;
        let mut resp_data = resp_body;
        let json_str = types::read_string(&mut resp_data, 32767).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(json["version"]["protocol"], constants::PROTOCOL_VERSION);

        let _ = shutdown_tx.send(());
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_listener_graceful_shutdown() {
        let ctx = test_login_context();
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tcp_listener = TcpListener::bind(addr).await.unwrap();
        let bound_addr = tcp_listener.local_addr().unwrap();
        drop(tcp_listener);

        let shutdown_rx = shutdown_tx.subscribe();
        let server = tokio::spawn(async move {
            listen(bound_addr, ctx, shutdown_rx).await.unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let _ = shutdown_tx.send(());

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), server).await;
        assert!(result.is_ok(), "server should shut down within 2 seconds");
    }
}
