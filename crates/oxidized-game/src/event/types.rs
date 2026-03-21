//! Event types, kinds, results, and handler signatures.

use uuid::Uuid;

/// Discriminant for event registration — identifies which event kind
/// a handler wants to receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// A player has joined the server.
    PlayerJoin,
    /// A player has left the server.
    PlayerQuit,
    /// A player sent a chat message.
    PlayerChat,
    /// A player executed a command.
    PlayerCommand,
    /// A player broke a block.
    BlockBreak,
    /// A player placed a block.
    BlockPlace,
    /// The server is shutting down.
    ServerShutdown,
}

/// Data payload for a game event.
///
/// Each variant carries the context needed for handlers to make
/// allow/deny decisions. Additional fields will be added as the
/// server implements more gameplay systems.
#[derive(Debug, Clone)]
pub enum GameEvent {
    /// A player joined the server.
    PlayerJoin {
        /// The player's UUID.
        uuid: Uuid,
        /// The player's name.
        name: String,
    },
    /// A player left the server.
    PlayerQuit {
        /// The player's UUID.
        uuid: Uuid,
        /// The player's name.
        name: String,
    },
    /// A player sent a chat message.
    PlayerChat {
        /// The player's UUID.
        uuid: Uuid,
        /// The player's name.
        name: String,
        /// The chat message content.
        message: String,
    },
    /// A player executed a command.
    PlayerCommand {
        /// The player's UUID.
        uuid: Uuid,
        /// The player's name.
        name: String,
        /// The command string (without leading `/`).
        command: String,
    },
    /// A player broke a block.
    BlockBreak {
        /// The player's UUID.
        uuid: Uuid,
    },
    /// A player placed a block.
    BlockPlace {
        /// The player's UUID.
        uuid: Uuid,
    },
    /// The server is shutting down.
    ServerShutdown,
}

impl GameEvent {
    /// Returns the discriminant [`EventKind`] for this event.
    pub fn kind(&self) -> EventKind {
        match self {
            Self::PlayerJoin { .. } => EventKind::PlayerJoin,
            Self::PlayerQuit { .. } => EventKind::PlayerQuit,
            Self::PlayerChat { .. } => EventKind::PlayerChat,
            Self::PlayerCommand { .. } => EventKind::PlayerCommand,
            Self::BlockBreak { .. } => EventKind::BlockBreak,
            Self::BlockPlace { .. } => EventKind::BlockPlace,
            Self::ServerShutdown => EventKind::ServerShutdown,
        }
    }
}

/// The result of handling an event.
///
/// Returned by each [`EventHandler`] to indicate whether the event
/// should proceed or be cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Allow the event to proceed normally.
    Allow,
    /// Cancel the event — the originating action should not happen.
    Deny,
}

/// Opaque identifier for a registered event handler.
///
/// Returned by [`EventBus::subscribe`] and used by
/// [`EventBus::unsubscribe`] to remove a specific handler.
///
/// [`EventBus::subscribe`]: super::EventBus::subscribe
/// [`EventBus::unsubscribe`]: super::EventBus::unsubscribe
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(pub(crate) u64);

/// A callback invoked when a matching event fires.
///
/// Must be `Send + Sync` because the event bus is shared across threads.
/// Return [`EventResult::Deny`] to cancel the event.
pub type EventHandler = Box<dyn Fn(&GameEvent) -> EventResult + Send + Sync>;
