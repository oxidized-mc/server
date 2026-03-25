//! Entity selector parsing and resolution (`@a`, `@e`, `@p`, `@r`, `@s`, `@n`).
//!
//! Parses vanilla-compatible filter syntax (`[key=value,…]`) and applies filters.
//! Filters that can be evaluated with the current player-only data model (`name`,
//! `limit`, `sort`, `gamemode`, `distance`, `type`) are applied during resolution.
//! Remaining filters (`tag`, `nbt`, `scores`, `advancements`, `team`, `level`,
//! rotations, coordinates/volumes) are parsed and stored for future ECS use.

use crate::commands::CommandError;
use crate::commands::argument_parser::parse_range;
use crate::commands::source::{CommandSourceKind, CommandSourceStack};
use crate::player::game_mode::GameMode;
use rand::RngExt;
use std::str::FromStr;

// ── Constants ──────────────────────────────────────────────────────────

/// All supported filter keys for tab-completion, in alphabetical order.
pub const FILTER_KEYS: &[&str] = &[
    "advancements",
    "distance",
    "dx",
    "dy",
    "dz",
    "gamemode",
    "level",
    "limit",
    "name",
    "nbt",
    "scores",
    "sort",
    "tag",
    "team",
    "type",
    "x",
    "x_rotation",
    "y",
    "y_rotation",
    "z",
];

/// Valid values for the `sort=` filter.
pub const SORT_VALUES: &[&str] = &["arbitrary", "furthest", "nearest", "random"];

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

impl FromStr for SelectorSort {
    type Err = CommandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "nearest" => Ok(Self::Nearest),
            "furthest" => Ok(Self::Furthest),
            "random" => Ok(Self::Random),
            "arbitrary" => Ok(Self::Arbitrary),
            _ => Err(CommandError::Parse(format!(
                "Invalid sort mode: '{s}' (expected nearest, furthest, random, or arbitrary)"
            ))),
        }
    }
}

/// A single name filter entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameFilter {
    /// The name to match.
    pub name: String,
    /// If `true`, this is an exclusion (`name=!X`).
    pub is_negated: bool,
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
                let (val, is_negated) = parse_negatable(value);
                filters.name.push(NameFilter {
                    name: val.to_string(),
                    is_negated,
                });
            },
            "limit" => {
                let n: u32 = value
                    .parse()
                    .map_err(|_| CommandError::Parse(format!("Invalid limit value: '{value}'")))?;
                if n == 0 {
                    return Err(CommandError::Parse("Limit must be at least 1".to_string()));
                }
                filters.limit = Some(n);
            },
            "sort" => {
                filters.sort = Some(value.parse()?);
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
                let (val, is_negated) = parse_negatable(value);
                filters.entity_type.push((val.to_string(), is_negated));
            },
            "tag" => {
                let (val, is_negated) = parse_negatable(value);
                filters.tag.push((val.to_string(), is_negated));
            },
            "team" => {
                let (val, is_negated) = parse_negatable(value);
                filters.team.push((val.to_string(), is_negated));
            },
            "gamemode" => {
                let (val, is_negated) = parse_negatable(value);
                filters.gamemode.push((val.to_string(), is_negated));
            },
            "nbt" => {
                let (val, is_negated) = parse_negatable(value);
                filters.nbt.push((val.to_string(), is_negated));
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
    let (min, max) = parse_range::<f64>(value, "double")?;
    Ok(DoubleRange { min, max })
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
///
/// Applied filters: `name`, `limit`, `sort`, `gamemode`, `distance`, `type`.
/// Stored but not yet enforced: `tag`, `nbt`, `scores`, `advancements`,
/// `team`, `level`, `x/y/z/dx/dy/dz`, rotations.
pub fn resolve_selector(
    selector: &EntitySelector,
    source: &CommandSourceStack,
) -> Result<Vec<SelectorTarget>, CommandError> {
    let kind = selector.kind;
    let filters = &selector.filters;

    let mut targets = match kind {
        SelectorKind::AllPlayers | SelectorKind::AllEntities => collect_all_players(source)?,
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
        if nf.is_negated {
            targets.retain(|t| t.name != nf.name);
        } else {
            targets.retain(|t| t.name == nf.name);
        }
    }

    // Apply gamemode filters.
    for (mode_name, is_negated) in &filters.gamemode {
        let mode = GameMode::from_name(mode_name).ok_or_else(|| {
            CommandError::Parse(format!("Invalid game mode: '{mode_name}'"))
        })?;
        targets.retain(|t| {
            let player_mode = source.server.get_player_game_mode(&t.uuid);
            match player_mode {
                Some(gm) => (gm == mode) != *is_negated,
                // If we can't determine the game mode, exclude the target.
                None => false,
            }
        });
    }

    // Apply type filters. Currently only players exist, so "player" /
    // "minecraft:player" matches and everything else excludes.
    for (type_name, is_negated) in &filters.entity_type {
        let is_player_type =
            type_name == "player" || type_name == "minecraft:player";
        if is_player_type == *is_negated {
            // type=player,negated → exclude all players → empty
            // type=!zombie → is_player=false, negated=true → keep all
            targets.clear();
        }
        // type=player (not negated) or type=!non_player → keep all current targets
    }

    // Apply distance filter using source position.
    if let Some(ref distance) = filters.distance {
        let (sx, sy, sz) = source.position;
        targets.retain(|t| {
            let pos = source.server.get_player_position(&t.uuid);
            match pos {
                Some((px, py, pz)) => {
                    let dx = px - sx;
                    let dy = py - sy;
                    let dz = pz - sz;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    if let Some(min) = distance.min {
                        if dist < min {
                            return false;
                        }
                    }
                    if let Some(max) = distance.max {
                        if dist > max {
                            return false;
                        }
                    }
                    true
                },
                None => false,
            }
        });
    }

    // Apply sort. Without positions, nearest/furthest use natural order.
    match filters.sort {
        Some(SelectorSort::Random) => shuffle(&mut targets),
        Some(SelectorSort::Furthest) => targets.reverse(),
        _ => {},
    }

    // Apply limit. Default limits depend on selector kind.
    let limit = filters.limit.unwrap_or(match kind {
        SelectorKind::NearestPlayer | SelectorKind::NearestEntity | SelectorKind::RandomPlayer => 1,
        _ => u32::MAX,
    }) as usize;

    targets.truncate(limit);

    if targets.is_empty() {
        return Err(CommandError::Execution("No entities found".to_string()));
    }

    Ok(targets)
}

/// Collects all online players as [`SelectorTarget`]s.
fn collect_all_players(source: &CommandSourceStack) -> Result<Vec<SelectorTarget>, CommandError> {
    let names = source.server.online_player_names();
    if names.is_empty() {
        return Err(CommandError::Execution("No players found".to_string()));
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
        assert!(!sel.filters.name[0].is_negated);
    }

    #[test]
    fn parse_selector_with_negated_name() {
        let sel = parse_selector("@a[name=!Steve]").unwrap();
        assert!(sel.filters.name[0].is_negated);
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
        assert_eq!(sel.filters.gamemode, vec![("creative".to_string(), false)]);
    }

    #[test]
    fn parse_selector_with_negated_gamemode() {
        let sel = parse_selector("@a[gamemode=!spectator]").unwrap();
        assert_eq!(sel.filters.gamemode, vec![("spectator".to_string(), true)]);
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

    // ── Resolution tests ──────────────────────────────────────────────

    use crate::commands::source::{CommandSourceKind, CommandSourceStack, ServerHandle};
    use crate::player::game_mode::GameMode as GM;
    use oxidized_protocol::chat::Component;
    use std::sync::Arc;

    /// Mock player data for resolution tests.
    struct MockPlayerData {
        name: String,
        uuid: uuid::Uuid,
        game_mode: GM,
        position: (f64, f64, f64),
    }

    /// Mock server that supports name, UUID, game mode, and position queries.
    struct MockServer {
        players: Vec<MockPlayerData>,
    }

    impl MockServer {
        fn new(players: Vec<MockPlayerData>) -> Self {
            Self { players }
        }
    }

    impl ServerHandle for MockServer {
        fn broadcast_to_ops(&self, _msg: &Component, _min_level: u32) {}
        fn request_shutdown(&self) {}
        fn seed(&self) -> i64 {
            0
        }
        fn online_player_names(&self) -> Vec<String> {
            self.players.iter().map(|p| p.name.clone()).collect()
        }
        fn online_player_count(&self) -> usize {
            self.players.len()
        }
        fn max_players(&self) -> usize {
            20
        }
        fn difficulty(&self) -> i32 {
            2
        }
        fn game_time(&self) -> i64 {
            0
        }
        fn day_time(&self) -> i64 {
            0
        }
        fn is_raining(&self) -> bool {
            false
        }
        fn is_thundering(&self) -> bool {
            false
        }
        fn kick_player(&self, _name: &str, _reason: &str) -> bool {
            false
        }
        fn find_player_uuid(&self, name: &str) -> Option<uuid::Uuid> {
            self.players.iter().find(|p| p.name == name).map(|p| p.uuid)
        }
        fn command_descriptions(&self) -> Vec<(String, Option<String>)> {
            vec![]
        }
        fn get_player_game_mode(&self, uuid: &uuid::Uuid) -> Option<GM> {
            self.players
                .iter()
                .find(|p| &p.uuid == uuid)
                .map(|p| p.game_mode)
        }
        fn get_player_position(
            &self,
            uuid: &uuid::Uuid,
        ) -> Option<(f64, f64, f64)> {
            self.players
                .iter()
                .find(|p| &p.uuid == uuid)
                .map(|p| p.position)
        }
    }

    fn make_source(server: Arc<dyn ServerHandle>) -> CommandSourceStack {
        let players = server.online_player_names();
        let (name, uuid) = if let Some(first) = players.first() {
            let uuid = server.find_player_uuid(first).unwrap();
            (first.clone(), uuid)
        } else {
            ("Console".to_string(), uuid::Uuid::nil())
        };
        CommandSourceStack {
            source: CommandSourceKind::Player {
                name: name.clone(),
                uuid,
            },
            position: (0.0, 64.0, 0.0),
            rotation: (0.0, 0.0),
            permission_level: 4,
            display_name: name,
            server,
            feedback_sender: Arc::new(|_| {}),
            is_silent: false,
        }
    }

    fn test_players() -> Vec<MockPlayerData> {
        vec![
            MockPlayerData {
                name: "Alice".into(),
                uuid: uuid::Uuid::from_u128(1),
                game_mode: GM::Creative,
                position: (10.0, 64.0, 10.0),
            },
            MockPlayerData {
                name: "Bob".into(),
                uuid: uuid::Uuid::from_u128(2),
                game_mode: GM::Survival,
                position: (100.0, 64.0, 0.0),
            },
            MockPlayerData {
                name: "Charlie".into(),
                uuid: uuid::Uuid::from_u128(3),
                game_mode: GM::Spectator,
                position: (0.0, 64.0, 0.0),
            },
        ]
    }

    #[test]
    fn resolve_gamemode_filter_includes() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        let sel = parse_selector("@a[gamemode=creative]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Alice");
    }

    #[test]
    fn resolve_gamemode_filter_excludes() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        let sel = parse_selector("@a[gamemode=!spectator]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
        assert!(!names.contains(&"Charlie"));
    }

    #[test]
    fn resolve_gamemode_invalid_mode_errors() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        let sel = parse_selector("@a[gamemode=hardcore]").unwrap();
        let result = resolve_selector(&sel, &src);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_distance_filter() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // Source at (0, 64, 0). Alice at (10, 64, 10) = ~14.1 blocks.
        // Bob at (100, 64, 0) = 100 blocks. Charlie at (0, 64, 0) = 0 blocks.
        let sel = parse_selector("@a[distance=..20]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Alice"), "Alice is ~14 blocks away");
        assert!(names.contains(&"Charlie"), "Charlie is 0 blocks away");
        assert!(!names.contains(&"Bob"), "Bob is 100 blocks away");
    }

    #[test]
    fn resolve_distance_filter_min_max() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // Only Bob at 100 blocks (Alice ~14, Charlie 0)
        let sel = parse_selector("@a[distance=50..200]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Bob");
    }

    #[test]
    fn resolve_type_filter_player() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        let sel = parse_selector("@e[type=minecraft:player]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn resolve_type_filter_excludes_non_player() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // type=zombie — no players match
        let sel = parse_selector("@e[type=zombie]").unwrap();
        let result = resolve_selector(&sel, &src);
        assert!(result.is_err()); // "No entities found"
    }

    #[test]
    fn resolve_type_filter_negated_player() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // type=!player — excludes all current entities (all are players)
        let sel = parse_selector("@e[type=!player]").unwrap();
        let result = resolve_selector(&sel, &src);
        assert!(result.is_err()); // "No entities found"
    }

    #[test]
    fn resolve_type_filter_negated_non_player() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // type=!zombie — excludes zombies, keeps all players
        let sel = parse_selector("@e[type=!zombie]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn resolve_combined_filters() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        // gamemode=survival + distance within 200
        let sel =
            parse_selector("@a[gamemode=survival,distance=..200]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Bob");
    }

    #[test]
    fn resolve_limit_with_gamemode() {
        let server = Arc::new(MockServer::new(test_players()));
        let src = make_source(server);
        let sel =
            parse_selector("@a[gamemode=!spectator,limit=1]").unwrap();
        let result = resolve_selector(&sel, &src).unwrap();
        assert_eq!(result.len(), 1);
    }
}
