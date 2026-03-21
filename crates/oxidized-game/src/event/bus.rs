//! Thread-safe event dispatcher.

use std::sync::atomic::{AtomicU64, Ordering};

use ahash::AHashMap;
use parking_lot::RwLock;

use super::types::{EventHandler, EventKind, EventResult, GameEvent, HandlerId};

/// Thread-safe event dispatcher.
///
/// Handlers are registered per [`EventKind`] and invoked in registration
/// order when an event fires. Any handler returning [`EventResult::Deny`]
/// short-circuits further processing.
///
/// # Thread Safety
///
/// Registration (`subscribe`/`unsubscribe`) takes a write lock.
/// Firing (`fire`) takes a read lock — multiple events can fire
/// concurrently as long as no registration is in progress.
///
/// # Example
///
/// ```
/// use oxidized_game::event::{EventBus, EventKind, EventResult, GameEvent};
///
/// let bus = EventBus::new();
///
/// let id = bus.subscribe(EventKind::PlayerChat, Box::new(|event| {
///     if let GameEvent::PlayerChat { message, .. } = event {
///         if message.contains("bad_word") {
///             return EventResult::Deny;
///         }
///     }
///     EventResult::Allow
/// }));
///
/// let event = GameEvent::PlayerChat {
///     uuid: uuid::Uuid::nil(),
///     name: "Steve".into(),
///     message: "hello".into(),
/// };
/// assert_eq!(bus.fire(&event), EventResult::Allow);
///
/// let blocked = GameEvent::PlayerChat {
///     uuid: uuid::Uuid::nil(),
///     name: "Steve".into(),
///     message: "bad_word".into(),
/// };
/// assert_eq!(bus.fire(&blocked), EventResult::Deny);
///
/// assert!(bus.unsubscribe(EventKind::PlayerChat, id));
/// ```
pub struct EventBus {
    handlers: RwLock<AHashMap<EventKind, Vec<(HandlerId, EventHandler)>>>,
    next_id: AtomicU64,
}

impl EventBus {
    /// Creates an empty event bus with no handlers.
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(AHashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Registers a handler for the given event kind.
    ///
    /// Returns a [`HandlerId`] that can be used to unregister the handler
    /// later via [`unsubscribe`](Self::unsubscribe).
    pub fn subscribe(&self, kind: EventKind, handler: EventHandler) -> HandlerId {
        let id = HandlerId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let mut map = self.handlers.write();
        map.entry(kind).or_default().push((id, handler));
        id
    }

    /// Removes a previously registered handler.
    ///
    /// Returns `true` if the handler was found and removed.
    pub fn unsubscribe(&self, kind: EventKind, id: HandlerId) -> bool {
        let mut map = self.handlers.write();
        if let Some(handlers) = map.get_mut(&kind) {
            let len_before = handlers.len();
            handlers.retain(|(hid, _)| *hid != id);
            handlers.len() < len_before
        } else {
            false
        }
    }

    /// Fires an event, invoking all registered handlers in registration order.
    ///
    /// Returns [`EventResult::Deny`] if any handler denied the event
    /// (short-circuiting remaining handlers), otherwise [`EventResult::Allow`].
    pub fn fire(&self, event: &GameEvent) -> EventResult {
        let map = self.handlers.read();
        if let Some(handlers) = map.get(&event.kind()) {
            for (_, handler) in handlers {
                if handler(event) == EventResult::Deny {
                    return EventResult::Deny;
                }
            }
        }
        EventResult::Allow
    }

    /// Returns the number of handlers registered for a given event kind.
    pub fn handler_count(&self, kind: EventKind) -> usize {
        let map = self.handlers.read();
        map.get(&kind).map_or(0, Vec::len)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: EventBus is Send + Sync because:
// - RwLock<AHashMap> is Send + Sync when the value is Send
// - EventHandler is Send + Sync by definition
// - AtomicU64 is Send + Sync
// These bounds are automatically derived, this comment is for documentation.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use uuid::Uuid;

    fn chat_event(msg: &str) -> GameEvent {
        GameEvent::PlayerChat {
            uuid: Uuid::nil(),
            name: "Steve".into(),
            message: msg.into(),
        }
    }

    #[test]
    fn test_fire_with_no_handlers_allows() {
        let bus = EventBus::new();
        assert_eq!(bus.fire(&chat_event("hello")), EventResult::Allow);
    }

    #[test]
    fn test_subscribe_and_fire_allow() {
        let bus = EventBus::new();
        bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Allow));
        assert_eq!(bus.fire(&chat_event("hello")), EventResult::Allow);
    }

    #[test]
    fn test_handler_can_deny() {
        let bus = EventBus::new();
        bus.subscribe(
            EventKind::PlayerChat,
            Box::new(|event| {
                if let GameEvent::PlayerChat { message, .. } = event {
                    if message.contains("spam") {
                        return EventResult::Deny;
                    }
                }
                EventResult::Allow
            }),
        );
        assert_eq!(bus.fire(&chat_event("hello")), EventResult::Allow);
        assert_eq!(bus.fire(&chat_event("buy spam now")), EventResult::Deny);
    }

    #[test]
    fn test_deny_short_circuits() {
        let bus = EventBus::new();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));

        // First handler denies everything.
        bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Deny));

        // Second handler should never run.
        let c = Arc::clone(&counter);
        bus.subscribe(
            EventKind::PlayerChat,
            Box::new(move |_| {
                c.fetch_add(1, Ordering::Relaxed);
                EventResult::Allow
            }),
        );

        assert_eq!(bus.fire(&chat_event("test")), EventResult::Deny);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_unsubscribe_removes_handler() {
        let bus = EventBus::new();
        let id = bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Deny));
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 1);

        assert!(bus.unsubscribe(EventKind::PlayerChat, id));
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 0);
        assert_eq!(bus.fire(&chat_event("hello")), EventResult::Allow);
    }

    #[test]
    fn test_unsubscribe_wrong_kind_returns_false() {
        let bus = EventBus::new();
        let id = bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Allow));
        assert!(!bus.unsubscribe(EventKind::PlayerJoin, id));
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 1);
    }

    #[test]
    fn test_unsubscribe_nonexistent_returns_false() {
        let bus = EventBus::new();
        assert!(!bus.unsubscribe(EventKind::PlayerChat, HandlerId(999)));
    }

    #[test]
    fn test_different_kinds_are_independent() {
        let bus = EventBus::new();
        bus.subscribe(EventKind::PlayerJoin, Box::new(|_| EventResult::Deny));
        // PlayerJoin handler should not affect PlayerChat events.
        assert_eq!(bus.fire(&chat_event("hello")), EventResult::Allow);
    }

    #[test]
    fn test_multiple_handlers_run_in_order() {
        let bus = EventBus::new();
        let order = Arc::new(parking_lot::Mutex::new(Vec::new()));

        let o1 = Arc::clone(&order);
        bus.subscribe(
            EventKind::PlayerChat,
            Box::new(move |_| {
                o1.lock().push(1);
                EventResult::Allow
            }),
        );

        let o2 = Arc::clone(&order);
        bus.subscribe(
            EventKind::PlayerChat,
            Box::new(move |_| {
                o2.lock().push(2);
                EventResult::Allow
            }),
        );

        bus.fire(&chat_event("test"));
        assert_eq!(*order.lock(), vec![1, 2]);
    }

    #[test]
    fn test_handler_count() {
        let bus = EventBus::new();
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 0);

        let id1 = bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Allow));
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 1);

        let _id2 = bus.subscribe(EventKind::PlayerChat, Box::new(|_| EventResult::Allow));
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 2);

        bus.unsubscribe(EventKind::PlayerChat, id1);
        assert_eq!(bus.handler_count(EventKind::PlayerChat), 1);
    }

    #[test]
    fn test_event_kind_discriminant() {
        let join = GameEvent::PlayerJoin {
            uuid: Uuid::nil(),
            name: "Alex".into(),
        };
        assert_eq!(join.kind(), EventKind::PlayerJoin);

        let quit = GameEvent::PlayerQuit {
            uuid: Uuid::nil(),
            name: "Alex".into(),
        };
        assert_eq!(quit.kind(), EventKind::PlayerQuit);

        let cmd = GameEvent::PlayerCommand {
            uuid: Uuid::nil(),
            name: "Alex".into(),
            command: "help".into(),
        };
        assert_eq!(cmd.kind(), EventKind::PlayerCommand);

        assert_eq!(GameEvent::ServerShutdown.kind(), EventKind::ServerShutdown);
    }

    #[test]
    fn test_concurrent_fire_is_safe() {
        let bus = Arc::new(EventBus::new());
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));

        let c = Arc::clone(&counter);
        bus.subscribe(
            EventKind::PlayerChat,
            Box::new(move |_| {
                c.fetch_add(1, Ordering::Relaxed);
                EventResult::Allow
            }),
        );

        let mut handles = Vec::new();
        for _ in 0..10 {
            let bus = Arc::clone(&bus);
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    bus.fire(&chat_event("concurrent"));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::Relaxed), 1000);
    }
}
