//! Command source — the identity and context of whoever runs a command.

use oxidized_protocol::chat::Component;
use std::sync::Arc;

/// A handle to the server, used by commands that need to affect global state.
pub trait ServerHandle: Send + Sync {
    /// Sends a message to ops (players with op level ≥ `min_level`).
    fn broadcast_to_ops(&self, message: &Component, min_level: u32);
    /// Requests a graceful server shutdown.
    fn request_shutdown(&self);
    /// Returns the server's seed.
    fn seed(&self) -> i64;
    /// Returns a list of online player names.
    fn online_player_names(&self) -> Vec<String>;
    /// Returns the number of online players.
    fn online_player_count(&self) -> usize;
    /// Returns the maximum number of players.
    fn max_players(&self) -> usize;
}

/// The kind of entity that is executing a command.
#[derive(Clone)]
pub enum CommandSourceKind {
    /// A connected player.
    Player {
        /// The player's display name.
        name: String,
        /// The player's UUID.
        uuid: uuid::Uuid,
    },
    /// The server console.
    Console,
}

/// Full execution context for a command, analogous to vanilla's
/// `CommandSourceStack`.
#[derive(Clone)]
pub struct CommandSourceStack {
    /// What kind of entity is running the command.
    pub source: CommandSourceKind,
    /// Position of the command source in world space.
    pub position: (f64, f64, f64),
    /// Rotation (yaw, pitch) of the command source.
    pub rotation: (f32, f32),
    /// Permission level (0-4).
    pub permission_level: u32,
    /// Display name of the source.
    pub display_name: String,
    /// Handle to the server for global operations.
    pub server: Arc<dyn ServerHandle>,
    /// A callback to send feedback messages to this source.
    pub feedback_sender: Arc<dyn Fn(&Component) + Send + Sync>,
    /// If true, suppress all feedback messages.
    pub silent: bool,
}

impl CommandSourceStack {
    /// Returns `true` if the source has at least the given permission level.
    pub fn has_permission(&self, level: u32) -> bool {
        self.permission_level >= level
    }

    /// Sends a success message to the source and optionally broadcasts to ops.
    pub fn send_success(&self, component: &Component, broadcast_to_ops: bool) {
        if !self.silent {
            (self.feedback_sender)(component);
        }
        if broadcast_to_ops {
            self.server.broadcast_to_ops(component, 1);
        }
    }

    /// Sends a failure message to the source.
    pub fn send_failure(&self, component: &Component) {
        (self.feedback_sender)(component);
    }

    /// Sends a system message to the source.
    pub fn send_message(&self, component: &Component) {
        (self.feedback_sender)(component);
    }
}
