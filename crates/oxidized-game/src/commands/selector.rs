//! Entity selector parsing and resolution (`@a`, `@e`, `@p`, `@r`, `@s`, `@n`).
//!
//! Parses vanilla-compatible filter syntax (`[key=value,…]`) and applies filters
//! that can be resolved with the current player-only data model. Filters that
//! require ECS entity queries (e.g. `distance`, `type`, `tag`, `nbt`, `scores`,
//! `advancements`) are parsed and stored but not yet applied during resolution.

use crate::commands::CommandError;
use crate::commands::source::{CommandSourceKind, CommandSourceStack};
use rand::RngExt;

// ── Types ──────────────────────────────────────────────────────────────

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

/// Sort order for selector results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorSort {
    /// No specific ordering.
    Arbitrary,
    /// Closest first (default for `@p`, `@n`).
    Nearest,
    /// Farthest first.
    Furthest,
    /// Random order (default for `@r`).
    Random,
}

/// A single name filter entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameFilter {
    /// The name to match.
    pub name: String,
    /// If `true`, this is an exclusion (`name=!X`).
    pub negated: bool,
}

/// A double-ended range used for `distance`, `level`, `x_rotation`, `y_rotation`.
#[derive(Debug, Clone, PartialEq)]
pub struct DoubleRange {
    /// Inclusive minimum (if any).
    pub min: Option<f64>,
    /// Inclusive maximum (if any).
    pub max: Option<f64>,
}

/// Filters parsed from `[key=value,…]` syntax.
///
/// Fields that are `None` / empty mean "no constraint". Filters that cannot
/// be resolved yet (distance, type, tag, nbt, scores, advancements, team,
/// gamemode, level, x/y/z/dx/dy/dz, rotations) are stored for future use.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectorFilters {
    /// `name=X` or `name=!X` — player name filters.
    pub name: Vec<NameFilter>,
    /// `limit=N` — maximum number of results.
    pub limit: Option<u32>,
    /// `sort=nearest|furthest|random|arbitrary`.
    pub sort: Option<SelectorSort>,
    /// `distance=min..max` — distance from source.
    pub distance: Option<DoubleRange>,
    /// `level=min..max` — experience level (players only).
    pub level: Option<DoubleRange>,
    /// `x_rotation=min..max` — pitch angle.
    pub x_rotation: Option<DoubleRange>,
    /// `y_rotation=min..max` — yaw angle.
    pub y_rotation: Option<DoubleRange>,
    /// `x=<double>` — anchor X coordinate.
    pub x: Option<f64>,
    /// `y=<double>` — anchor Y coordinate.
    pub y: Option<f64>,
    /// `z=<double>` — anchor Z coordinate.
    pub z: Option<f64>,
    /// `dx=<double>` — bounding box delta X.
    pub dx: Option<f64>,
    /// `dy=<double>` — bounding box delta Y.
    pub dy: Option<f64>,
    /// `dz=<double>` — bounding box delta Z.
    pub dz: Option<f64>,
    /// `type=X` or `type=!X` — entity type filter string.
    pub entity_type: Vec<(String, bool)>,
    /// `tag=X` or `tag=!X` — entity tag filters.
    pub tag: Vec<(String, bool)>,
    /// `team=X` or `team=!X` — scoreboard team filters.
    pub team: Vec<(String, bool)>,
    /// `gamemode=X` or `gamemode=!X` — gamemode filters.
    pub gamemode: Vec<(String, bool)>,
    /// `nbt=<compound>` or `nbt=!<compound>` — NBT predicate (raw string).
    pub nbt: Vec<(String, bool)>,
    /// `scores={obj=min..max,…}` — scoreboard score filters (raw string).
    pub scores: Option<String>,
    /// `advancements={…}` — advancement filters (raw string).
    pub advancements: Option<String>,
}

/// A parsed entity selector: kind + optional filters.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySelector {
    /// The base selector kind.
    pub kind: SelectorKind,
    /// Parsed filter options.
    pub filters: SelectorFilters,
}

/// A resolved entity target from a selector.
#[derive(Debug, Clone)]
pub struct SelectorTarget {
    /// Display name of the target.
    pub name: String,
    /// UUID of the target.
    pub uuid: uuid::Uuid,
}

// ── Parsing ────────────────────────────────────────────────────────────

/// Parses a complete entity selector from the input string, including
/// optional `[key=value,…]` filters.
///
/// Returns `None` if the input is not a valid selector.
pub fn parse_selector(input: &str) -> Option<EntitySelector> {
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'@') || bytes.len() < 2 {
        return None;
    }

    let kind = match bytes[1] {
        b'a' => SelectorKind::AllPlayers,
        b'e' => SelectorKind::AllEntities,
        b'p' => SelectorKind::NearestPlayer,
        b'r' => SelectorKind::RandomPlayer,
        b's' => SelectorKind::SelfEntity,
        b'n' => SelectorKind::NearestEntity,
        _ => return None,
    };

    let rest = &input[2..];
    if rest.is_empty() {
        return Some(EntitySelector {
            kind,
            filters: SelectorFilters::default(),
        });
    }

    if !rest.starts_with('[') {
        return None;
    }

    // Must end with ']'.
    if !rest.ends_with(']') {
        return None;
    }

    let inner = &rest[1..rest.len() - 1];
    let filters = parse_filters(inner).ok()?;

    Some(EntitySelector { kind, filters })
}

/// Parses the inner content of `[…]` into [`SelectorFilters`].
fn parse_filters(inner: &str) -> Result<SelectorFilters, CommandError> {
    let mut filters = SelectorFilters::default();

    if inner.is_empty() {
        return Ok(filters);
    }

    for pair in split_filter_pairs(inner) {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| CommandError::Parse(format!("Invalid selector option: '{pair}'")))?;

        let key = key.trim();
        let value = value.trim();

        match key {
            "name" => {
                let (val, negated) = parse_negatable(value);
                filters.name.push(NameFilter {
                    name: val.to_string(),
                    negated,
                });
            },
            "limit" => {
                let n: u32 = value.parse().map_err(|_| {
                    CommandError::Parse(format!("Invalid limit value: '{value}'"))
                })?;
                if n == 0 {
                    return Err(CommandError::Parse("Limit must be at least 1".to_string()));
                }
                filters.limit = Some(n);
            },
            "sort" => {
                filters.sort = Some(match value {
                    "nearest" => SelectorSort::Nearest,
                    "furthest" => SelectorSort::Furthest,
                    "random" => SelectorSort::Random,
                    "arbitrary" => SelectorSort::Arbitrary,
                    _ => {
                        return Err(CommandError::Parse(format!(
                            "Invalid sort mode: '{value}'"
                        )));
                    },
                });
            },
            "distance" => filters.distance = Some(parse_double_range(value)?),
            "level" => filters.level = Some(parse_double_range(value)?),
            "x_rotation" => filters.x_rotation = Some(parse_double_range(value)?),
            "y_rotation" => filters.y_rotation = Some(parse_double_range(value)?),
            "x" => filters.x = Some(parse_f64(value, "x")?),
            "y" => filters.y = Some(parse_f64(value, "y")?),
            "z" => filters.z = Some(parse_f64(value, "z")?),
            "dx" => filters.dx = Some(parse_f64(value, "dx")?),
            "dy" => filters.dy = Some(parse_f64(value, "dy")?),
            "dz" => filters.dz = Some(parse_f64(value, "dz")?),
            "type" => {
                let (val, negated) = parse_negatable(value);
                filters.entity_type.push((val.to_string(), negated));
            },
            "tag" => {
                let (val, negated) = parse_negatable(value);
                filters.tag.push((val.to_string(), negated));
            },
            "team" => {
                let (val, negated) = parse_negatable(value);
                filters.team.push((val.to_string(), negated));
            },
            "gamemode" => {
                let (val, negated) = parse_negatable(value);
                filters.gamemode.push((val.to_string(), negated));
            },
            "nbt" => {
                let (val, negated) = parse_negatable(value);
                filters.nbt.push((val.to_string(), negated));
            },
            "scores" => filters.scores = Some(value.to_string()),
            "advancements" => filters.advancements = Some(value.to_string()),
            _ => {
                return Err(CommandError::Parse(format!(
                    "Unknown selector option: '{key}'"
                )));
            },
        }
    }

    Ok(filters)
}

/// Splits filter pairs by commas, respecting `{…}` nesting (for `scores=`,
/// `advancements=`).
fn split_filter_pairs(input: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;

    for (i, ch) in input.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                results.push(&input[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }
    results.push(&input[start..]);
    results
}

/// Checks if a value is negated with a `!` prefix.
fn parse_negatable(value: &str) -> (&str, bool) {
    if let Some(stripped) = value.strip_prefix('!') {
        (stripped, true)
    } else {
        (value, false)
    }
}

/// Parses a `min..max` range, supporting open-ended forms like `..10`, `5..`,
/// and single values like `10` (which means `10..10`).
fn parse_double_range(value: &str) -> Result<DoubleRange, CommandError> {
    if let Some((min_s, max_s)) = value.split_once("..") {
        let min = if min_s.is_empty() {
            None
        } else {
            Some(min_s.parse::<f64>().map_err(|_| {
                CommandError::Parse(format!("Invalid range minimum: '{min_s}'"))
            })?)
        };
        let max = if max_s.is_empty() {
            None
        } else {
            Some(max_s.parse::<f64>().map_err(|_| {
                CommandError::Parse(format!("Invalid range maximum: '{max_s}'"))
            })?)
        };
        Ok(DoubleRange { min, max })
    } else {
        // Single value means exact match.
        let v = value.parse::<f64>().map_err(|_| {
            CommandError::Parse(format!("Invalid range value: '{value}'"))
        })?;
        Ok(DoubleRange {
            min: Some(v),
            max: Some(v),
        })
    }
}

/// Parses a single f64 value for coordinate options.
fn parse_f64(value: &str, name: &str) -> Result<f64, CommandError> {
    value
        .parse::<f64>()
        .map_err(|_| CommandError::Parse(format!("Invalid {name} value: '{value}'")))
}

// ── Resolution ─────────────────────────────────────────────────────────

/// Resolves an [`EntitySelector`] against the current [`CommandSourceStack`].
///
/// Returns a list of matching [`SelectorTarget`]s. Only online players are
/// considered since the ECS entity layer is not yet wired into commands.
/// Filters that can be applied with player data (`name`, `limit`, `sort`)
/// are applied. Filters requiring ECS data are stored but not yet enforced.
pub fn resolve_selector(
    selector: &EntitySelector,
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    let kind = selector.kind;
    let filters = &selector.filters;

    let mut targets = match kind {
        SelectorKind::AllPlayers | SelectorKind::AllEntities => {
            collect_all_players(source)?
        },
        SelectorKind::NearestPlayer | SelectorKind::NearestEntity => {
            // Without entity positions, return the executing player when
            // the source is a player, otherwise fall back to the first online.
            if let CommandSourceKind::Player { ref name, uuid } = source.source {
                vec![SelectorTarget {
                    name: name.clone(),
                    uuid,
                }]
            } else {
                let all = collect_all_players(source)?;
                // @p/@n default limit=1
                all.into_iter().take(1).collect()
            }
        },
        SelectorKind::RandomPlayer => {
            let mut all = collect_all_players(source)?;
            shuffle(&mut all);
            all
        },
        SelectorKind::SelfEntity => {
            if let CommandSourceKind::Player { ref name, uuid } = source.source {
                vec![SelectorTarget {
                    name: name.clone(),
                    uuid,
                }]
            } else {
                return Err(CommandError::Execution(
                    "This selector requires an entity, but the command source is the console"
                        .to_string(),
                ));
            }
        },
    };

    // Apply name filters.
    for nf in &filters.name {
        if nf.negated {
            targets.retain(|t| t.name != nf.name);
        } else {
            targets.retain(|t| t.name == nf.name);
        }
    }

    // Apply sort. Without positions, nearest/furthest use natural order.
    match filters.sort {
        Some(SelectorSort::Random) => shuffle(&mut targets),
        Some(SelectorSort::Furthest) => targets.reverse(),
        _ => {},
    }

    // Apply limit. Default limits depend on selector kind.
    let limit = filters.limit.unwrap_or(match kind {
        SelectorKind::NearestPlayer
        | SelectorKind::NearestEntity
        | SelectorKind::RandomPlayer => 1,
        _ => u32::MAX,
    }) as usize;

    targets.truncate(limit);

    if targets.is_empty() {
        return Err(CommandError::Execution("No entities found".to_string()));
    }

    Ok(targets)
}

/// Collects all online players as [`SelectorTarget`]s.
fn collect_all_players(
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    let names = source.server.online_player_names();
    if names.is_empty() {
        return Err(CommandError::Execution("No players found".to_string()));
    }
    let mut targets = Vec::with_capacity(names.len());
    for name in &names {
        if let Some(uuid) = source.server.find_player_uuid(name) {
            targets.push(SelectorTarget { name: name.clone(), uuid });
        }
    }
    Ok(targets)
}

/// Fisher-Yates shuffle.
fn shuffle(targets: &mut [SelectorTarget]) {
    let mut rng = rand::rng();
    for i in (1..targets.len()).rev() {
        let j = rng.random_range(0..=i);
        targets.swap(i, j);
    }
}

// ── Public convenience API ─────────────────────────────────────────────

/// Tries to resolve the given argument string as an entity selector or
/// player name. Returns a list of matching targets.
pub fn resolve_entities(
    input: &str,
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    if let Some(selector) = parse_selector(input) {
        resolve_selector(&selector, source)
    } else {
        // Try as a literal player name.
        if let Some(uuid) = source.server.find_player_uuid(input) {
            Ok(vec![SelectorTarget {
                name: input.to_string(),
                uuid,
            }])
        } else {
            Err(CommandError::Parse(format!("No player found: '{input}'")))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ── parse_selector (basic kind) ────────────────────────────────

    #[test]
    fn parse_selector_all_players() {
        let sel = parse_selector("@a").unwrap();
        assert_eq!(sel.kind, SelectorKind::AllPlayers);
        assert_eq!(sel.filters, SelectorFilters::default());
    }

    #[test]
    fn parse_selector_all_entities() {
        let sel = parse_selector("@e").unwrap();
        assert_eq!(sel.kind, SelectorKind::AllEntities);
    }

    #[test]
    fn parse_selector_nearest_player() {
        let sel = parse_selector("@p").unwrap();
        assert_eq!(sel.kind, SelectorKind::NearestPlayer);
    }

    #[test]
    fn parse_selector_random() {
        let sel = parse_selector("@r").unwrap();
        assert_eq!(sel.kind, SelectorKind::RandomPlayer);
    }

    #[test]
    fn parse_selector_self_entity() {
        let sel = parse_selector("@s").unwrap();
        assert_eq!(sel.kind, SelectorKind::SelfEntity);
    }

    #[test]
    fn parse_selector_nearest_entity() {
        let sel = parse_selector("@n").unwrap();
        assert_eq!(sel.kind, SelectorKind::NearestEntity);
    }

    #[test]
    fn parse_selector_invalid() {
        assert!(parse_selector("@x").is_none());
        assert!(parse_selector("notaselector").is_none());
        assert!(parse_selector("@").is_none());
        assert!(parse_selector("").is_none());
    }

    #[test]
    fn parse_selector_rejects_trailing_text() {
        assert!(parse_selector("@afoo").is_none());
    }

    // ── Filter parsing ─────────────────────────────────────────────

    #[test]
    fn parse_selector_with_name_filter() {
        let sel = parse_selector("@a[name=Steve]").unwrap();
        assert_eq!(sel.kind, SelectorKind::AllPlayers);
        assert_eq!(sel.filters.name.len(), 1);
        assert_eq!(sel.filters.name[0].name, "Steve");
        assert!(!sel.filters.name[0].negated);
    }

    #[test]
    fn parse_selector_with_negated_name() {
        let sel = parse_selector("@a[name=!Steve]").unwrap();
        assert!(sel.filters.name[0].negated);
        assert_eq!(sel.filters.name[0].name, "Steve");
    }

    #[test]
    fn parse_selector_with_limit() {
        let sel = parse_selector("@a[limit=3]").unwrap();
        assert_eq!(sel.filters.limit, Some(3));
    }

    #[test]
    fn parse_selector_limit_zero_rejected() {
        assert!(parse_selector("@a[limit=0]").is_none());
    }

    #[test]
    fn parse_selector_with_sort() {
        let sel = parse_selector("@e[sort=nearest]").unwrap();
        assert_eq!(sel.filters.sort, Some(SelectorSort::Nearest));

        let sel = parse_selector("@e[sort=furthest]").unwrap();
        assert_eq!(sel.filters.sort, Some(SelectorSort::Furthest));

        let sel = parse_selector("@e[sort=random]").unwrap();
        assert_eq!(sel.filters.sort, Some(SelectorSort::Random));

        let sel = parse_selector("@e[sort=arbitrary]").unwrap();
        assert_eq!(sel.filters.sort, Some(SelectorSort::Arbitrary));
    }

    #[test]
    fn parse_selector_with_distance_range() {
        let sel = parse_selector("@a[distance=..10]").unwrap();
        let d = sel.filters.distance.unwrap();
        assert_eq!(d.min, None);
        assert_eq!(d.max, Some(10.0));
    }

    #[test]
    fn parse_selector_with_distance_open_min() {
        let sel = parse_selector("@e[distance=5..]").unwrap();
        let d = sel.filters.distance.unwrap();
        assert_eq!(d.min, Some(5.0));
        assert_eq!(d.max, None);
    }

    #[test]
    fn parse_selector_with_distance_both() {
        let sel = parse_selector("@e[distance=10..50]").unwrap();
        let d = sel.filters.distance.unwrap();
        assert_eq!(d.min, Some(10.0));
        assert_eq!(d.max, Some(50.0));
    }

    #[test]
    fn parse_selector_with_exact_distance() {
        let sel = parse_selector("@e[distance=5]").unwrap();
        let d = sel.filters.distance.unwrap();
        assert_eq!(d.min, Some(5.0));
        assert_eq!(d.max, Some(5.0));
    }

    #[test]
    fn parse_selector_with_type_filter() {
        let sel = parse_selector("@e[type=zombie]").unwrap();
        assert_eq!(sel.filters.entity_type, vec![("zombie".to_string(), false)]);
    }

    #[test]
    fn parse_selector_with_negated_type() {
        let sel = parse_selector("@e[type=!player]").unwrap();
        assert_eq!(sel.filters.entity_type, vec![("player".to_string(), true)]);
    }

    #[test]
    fn parse_selector_multiple_filters() {
        let sel = parse_selector("@a[name=Steve,limit=2,sort=nearest]").unwrap();
        assert_eq!(sel.filters.name[0].name, "Steve");
        assert_eq!(sel.filters.limit, Some(2));
        assert_eq!(sel.filters.sort, Some(SelectorSort::Nearest));
    }

    #[test]
    fn parse_selector_with_gamemode() {
        let sel = parse_selector("@a[gamemode=creative]").unwrap();
        assert_eq!(
            sel.filters.gamemode,
            vec![("creative".to_string(), false)]
        );
    }

    #[test]
    fn parse_selector_with_negated_gamemode() {
        let sel = parse_selector("@a[gamemode=!spectator]").unwrap();
        assert_eq!(
            sel.filters.gamemode,
            vec![("spectator".to_string(), true)]
        );
    }

    #[test]
    fn parse_selector_with_coordinates() {
        let sel = parse_selector("@e[x=0,y=64,z=0,dx=10,dy=5,dz=10]").unwrap();
        assert_eq!(sel.filters.x, Some(0.0));
        assert_eq!(sel.filters.y, Some(64.0));
        assert_eq!(sel.filters.z, Some(0.0));
        assert_eq!(sel.filters.dx, Some(10.0));
        assert_eq!(sel.filters.dy, Some(5.0));
        assert_eq!(sel.filters.dz, Some(10.0));
    }

    #[test]
    fn parse_selector_with_scores() {
        let sel = parse_selector("@a[scores={health=10..20,points=0..}]").unwrap();
        assert_eq!(
            sel.filters.scores,
            Some("{health=10..20,points=0..}".to_string())
        );
    }

    #[test]
    fn parse_selector_with_tag() {
        let sel = parse_selector("@e[tag=example]").unwrap();
        assert_eq!(sel.filters.tag, vec![("example".to_string(), false)]);
    }

    #[test]
    fn parse_selector_empty_brackets() {
        let sel = parse_selector("@a[]").unwrap();
        assert_eq!(sel.filters, SelectorFilters::default());
    }

    #[test]
    fn parse_selector_with_level_range() {
        let sel = parse_selector("@a[level=10..50]").unwrap();
        let l = sel.filters.level.unwrap();
        assert_eq!(l.min, Some(10.0));
        assert_eq!(l.max, Some(50.0));
    }

    #[test]
    fn parse_selector_with_rotation() {
        let sel = parse_selector("@a[x_rotation=-90..90,y_rotation=0..360]").unwrap();
        let xr = sel.filters.x_rotation.unwrap();
        assert_eq!(xr.min, Some(-90.0));
        assert_eq!(xr.max, Some(90.0));
        let yr = sel.filters.y_rotation.unwrap();
        assert_eq!(yr.min, Some(0.0));
        assert_eq!(yr.max, Some(360.0));
    }

    #[test]
    fn parse_selector_unknown_option_rejected() {
        assert!(parse_selector("@a[foobar=1]").is_none());
    }

    #[test]
    fn parse_selector_invalid_sort_rejected() {
        assert!(parse_selector("@a[sort=sideways]").is_none());
    }

    // ── Range parsing ──────────────────────────────────────────────

    #[test]
    fn double_range_open_ended() {
        let r = parse_double_range("..100").unwrap();
        assert_eq!(r.min, None);
        assert_eq!(r.max, Some(100.0));
    }

    #[test]
    fn double_range_min_only() {
        let r = parse_double_range("5..").unwrap();
        assert_eq!(r.min, Some(5.0));
        assert_eq!(r.max, None);
    }

    #[test]
    fn double_range_exact() {
        let r = parse_double_range("42").unwrap();
        assert_eq!(r.min, Some(42.0));
        assert_eq!(r.max, Some(42.0));
    }

    #[test]
    fn double_range_both_bounds() {
        let r = parse_double_range("1.5..3.5").unwrap();
        assert_eq!(r.min, Some(1.5));
        assert_eq!(r.max, Some(3.5));
    }

    #[test]
    fn double_range_invalid() {
        assert!(parse_double_range("abc..10").is_err());
        assert!(parse_double_range("10..abc").is_err());
        assert!(parse_double_range("not_a_number").is_err());
    }
}
