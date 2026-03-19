//! Entity selector parsing and resolution (@a, @e, @p, @r, @s, @n).
//!
//! This is a basic implementation that resolves selectors to players from
//! the online player list. Filter syntax (`[key=value,...]`) is deferred
//! to a future phase.

use crate::commands::source::{CommandSourceKind, CommandSourceStack};
use crate::commands::CommandError;

/// The type of entity selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorKind {
    /// `@a` — all players.
    AllPlayers,
    /// `@e` — all entities (currently only players).
    AllEntities,
    /// `@p` — nearest player.
    NearestPlayer,
    /// `@r` — random player.
    RandomPlayer,
    /// `@s` — the executing entity.
    SelfEntity,
    /// `@n` — nearest entity (currently only players).
    NearestEntity,
}

/// A resolved entity target from a selector.
#[derive(Debug, Clone)]
pub struct SelectorTarget {
    /// Display name of the target.
    pub name: String,
    /// UUID of the target.
    pub uuid: uuid::Uuid,
}

/// Parses a selector prefix from the input string.
///
/// Returns the [`SelectorKind`] if the input starts with `@` followed by one
/// of `a`, `e`, `p`, `r`, `s`, `n`. Trailing `[…]` filter syntax is accepted
/// but ignored (future phase).
pub fn parse_selector(input: &str) -> Option<SelectorKind> {
    let mut chars = input.chars();
    if chars.next() != Some('@') {
        return None;
    }
    let kind_char = chars.next()?;
    let kind = match kind_char {
        'a' => SelectorKind::AllPlayers,
        'e' => SelectorKind::AllEntities,
        'p' => SelectorKind::NearestPlayer,
        'r' => SelectorKind::RandomPlayer,
        's' => SelectorKind::SelfEntity,
        'n' => SelectorKind::NearestEntity,
        _ => return None,
    };

    // The selector is valid if there is nothing after the kind character,
    // or if a filter bracket follows (which we skip for now).
    match chars.next() {
        None | Some('[') => Some(kind),
        _ => None,
    }
}

/// Resolves a [`SelectorKind`] against the current [`CommandSourceStack`].
///
/// Returns a list of matching [`SelectorTarget`]s. Only online players are
/// considered since the ECS entity layer is not yet wired into commands.
pub fn resolve_selector(
    kind: SelectorKind,
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    match kind {
        SelectorKind::AllPlayers | SelectorKind::AllEntities => {
            let names = source.server.online_player_names();
            if names.is_empty() {
                return Err(CommandError::Execution(
                    "No players found".to_string(),
                ));
            }
            let mut targets = Vec::with_capacity(names.len());
            for name in &names {
                if let Some(uuid) = source.server.find_player_uuid(name) {
                    targets.push(SelectorTarget {
                        name: name.clone(),
                        uuid,
                    });
                }
            }
            Ok(targets)
        },
        SelectorKind::NearestPlayer | SelectorKind::NearestEntity => {
            // Without entity positions in the command layer, return the
            // executing player when the source is a player, otherwise
            // fall back to the first online player.
            if let CommandSourceKind::Player { ref name, uuid } = source.source {
                return Ok(vec![SelectorTarget {
                    name: name.clone(),
                    uuid,
                }]);
            }
            let names = source.server.online_player_names();
            let first = names.first().ok_or_else(|| {
                CommandError::Execution("No players found".to_string())
            })?;
            let uuid = source.server.find_player_uuid(first).ok_or_else(|| {
                CommandError::Execution(format!("Could not resolve UUID for '{first}'"))
            })?;
            Ok(vec![SelectorTarget {
                name: first.clone(),
                uuid,
            }])
        },
        SelectorKind::RandomPlayer => {
            // TODO: Use a proper random source once `rand` is available.
            // For now, deterministically return the first online player.
            let names = source.server.online_player_names();
            let first = names.first().ok_or_else(|| {
                CommandError::Execution("No players found".to_string())
            })?;
            let uuid = source.server.find_player_uuid(first).ok_or_else(|| {
                CommandError::Execution(format!("Could not resolve UUID for '{first}'"))
            })?;
            Ok(vec![SelectorTarget {
                name: first.clone(),
                uuid,
            }])
        },
        SelectorKind::SelfEntity => {
            if let CommandSourceKind::Player { ref name, uuid } = source.source {
                Ok(vec![SelectorTarget {
                    name: name.clone(),
                    uuid,
                }])
            } else {
                Err(CommandError::Execution(
                    "This selector requires an entity, but the command source is the console"
                        .to_string(),
                ))
            }
        },
    }
}

/// Tries to resolve the given argument string as an entity selector or
/// player name. Returns a list of matching targets.
pub fn resolve_entities(
    input: &str,
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    if let Some(kind) = parse_selector(input) {
        resolve_selector(kind, source)
    } else {
        // Try as a literal player name.
        if let Some(uuid) = source.server.find_player_uuid(input) {
            Ok(vec![SelectorTarget {
                name: input.to_string(),
                uuid,
            }])
        } else {
            Err(CommandError::Parse(format!(
                "No player found: '{input}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn parse_selector_all_players() {
        assert_eq!(parse_selector("@a"), Some(SelectorKind::AllPlayers));
    }

    #[test]
    fn parse_selector_all_entities() {
        assert_eq!(parse_selector("@e"), Some(SelectorKind::AllEntities));
    }

    #[test]
    fn parse_selector_nearest_player() {
        assert_eq!(parse_selector("@p"), Some(SelectorKind::NearestPlayer));
    }

    #[test]
    fn parse_selector_random() {
        assert_eq!(parse_selector("@r"), Some(SelectorKind::RandomPlayer));
    }

    #[test]
    fn parse_selector_self_entity() {
        assert_eq!(parse_selector("@s"), Some(SelectorKind::SelfEntity));
    }

    #[test]
    fn parse_selector_nearest_entity() {
        assert_eq!(parse_selector("@n"), Some(SelectorKind::NearestEntity));
    }

    #[test]
    fn parse_selector_invalid() {
        assert_eq!(parse_selector("@x"), None);
        assert_eq!(parse_selector("notaselector"), None);
        assert_eq!(parse_selector("@"), None);
        assert_eq!(parse_selector(""), None);
    }

    #[test]
    fn parse_selector_with_filter_bracket() {
        // @a[distance=..10] should still parse as AllPlayers
        assert_eq!(
            parse_selector("@a[distance=..10]"),
            Some(SelectorKind::AllPlayers)
        );
    }

    #[test]
    fn parse_selector_rejects_trailing_text() {
        // @afoo is not a valid selector — extra chars after the kind
        assert_eq!(parse_selector("@afoo"), None);
    }
}
