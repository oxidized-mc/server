//! Game event system for plugin extensibility.
//!
//! Provides a thread-safe [`EventBus`] that dispatches [`GameEvent`]s to
//! registered handlers. Handlers can observe events or cancel them by
//! returning [`EventResult::Deny`].
//!
//! # Architecture
//!
//! The event bus is designed as a synchronous, in-process dispatcher:
//!
//! - **Registration** happens at startup (or when plugins load).
//! - **Firing** happens on hot paths (packet handlers, tick systems).
//! - Handlers run synchronously in registration order; a [`Deny`] result
//!   short-circuits remaining handlers.
//!
//! This module lives in `oxidized-game` so both the server binary and
//! future plugins can depend on it without pulling in `oxidized-server`.
//!
//! [`Deny`]: EventResult::Deny

mod bus;
mod types;

pub use bus::EventBus;
pub use types::{EventHandler, EventKind, EventResult, GameEvent, HandlerId};
