//! Command source — the identity and context of whoever runs a command.

use crate::event::EventBus;
use crate::level::weather::WeatherType;
use oxidized_protocol::chat::Component;
use std::sync::Arc;

/// A handle to the server, used by commands that need to affect global state.
///
/// This trait is also the primary extension point for future plugin support.
/// Plugins interact with the server through `Arc<dyn ServerHandle>` rather
/// than depending on concrete server internals.
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
    /// Returns the current difficulty (0=peaceful, 1=easy, 2=normal, 3=hard).
    fn difficulty(&self) -> i32;
    /// Returns the current game time in ticks.
    fn game_time(&self) -> i64;
    /// Returns the current day time in ticks.
    fn day_time(&self) -> i64;
    /// Returns true if raining.
    fn is_raining(&self) -> bool;
    /// Returns true if thundering.
    fn is_thundering(&self) -> bool;
    /// Kicks a player by name with a reason message. Returns true if found.
    fn kick_player(&self, name: &str, reason: &str) -> bool;
    /// Finds a player UUID by name.
    fn find_player_uuid(&self, name: &str) -> Option<uuid::Uuid>;
    /// Returns registered command names with optional descriptions, sorted.
    fn command_descriptions(&self) -> Vec<(String, Option<String>)>;

    // --- Plugin-ready extension points (default impls for backward compat) ---

    /// Returns the server's event bus for subscribing to game events.
    ///
    /// The default implementation returns `None`. Server implementations
    /// should override this to provide the real event bus.
    fn event_bus(&self) -> Option<&EventBus> {
        None
    }

    /// Broadcasts a chat message to all connected players.
    ///
    /// The default implementation does nothing. Override in the server
    /// to send via the broadcast channel.
    fn broadcast_chat(&self, _message: &Component) {}

    /// Sets the overworld day time to an absolute tick value.
    fn set_day_time(&self, _time: i64) {}

    /// Adds `ticks` to the current day time (may be negative).
    fn add_day_time(&self, _ticks: i64) {}

    /// Sets the weather state and optional duration in ticks.
    fn set_weather(&self, _weather: WeatherType, _duration: Option<i32>) {}

    /// Returns the string value of a game rule by its camelCase vanilla name.
    fn get_game_rule(&self, _name: &str) -> Option<String> {
        None
    }

    /// Sets a game rule from a string. Returns `Err` on invalid name or value.
    fn set_game_rule(&self, _name: &str, _value: &str) -> Result<(), String> {
        Err("game rules not supported".to_string())
    }

    /// Returns all game rule names sorted alphabetically.
    fn game_rule_names(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Returns the current tick rate in TPS.
    fn tick_rate(&self) -> f32 {
        20.0
    }

    /// Sets the server tick rate. Returns `true` if changed.
    fn set_tick_rate(&self, _rate: f32) -> bool {
        false
    }

    /// Returns `true` if the server tick loop is frozen.
    fn is_tick_frozen(&self) -> bool {
        false
    }

    /// Freezes or unfreezes the server tick loop.
    fn set_tick_frozen(&self, _frozen: bool) {}

    /// Requests N tick steps while frozen.
    fn tick_step(&self, _steps: u32) {}

    /// Returns remaining tick steps (0 if not stepping).
    fn tick_steps_remaining(&self) -> u32 {
        0
    }

    /// Starts a tick sprint for `ticks` duration.
    fn tick_sprint(&self, _ticks: u64) {}

    /// Returns `true` if the server is currently sprinting.
    fn is_tick_sprinting(&self) -> bool {
        false
    }

    /// Broadcasts the current tick rate and frozen state to all clients.
    fn broadcast_tick_state(&self) {}
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
