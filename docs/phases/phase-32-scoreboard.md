# Phase 32 — Scoreboard

**Status:** 📋 Planned  
**Crate:** `oxidized-game`  
**Reward:** `/scoreboard`, `/team`, `/bossbar` commands work; scores show in tab list.

---

## Architecture Decisions

No phase-specific ADRs. See [ADR index](../adr/README.md) for general architecture decisions.


## Goal

Implement the full scoreboard system: objectives with criteria and display slots,
per-player scores, team configuration (color, prefix, suffix, collision,
visibility), boss bar management, and all five wire packets. The system must
persist to and load from `data/scoreboard.dat` (gzip'd NBT), update clients in
real time as scores change, and support all `/scoreboard`, `/team`, and
`/bossbar` sub-commands.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Scoreboard | `Scoreboard` | `net.minecraft.world.scores.Scoreboard` |
| Objective | `Objective` | `net.minecraft.world.scores.Objective` |
| Player team | `PlayerTeam` | `net.minecraft.world.scores.PlayerTeam` |
| Player score | `ScoreAccess` | `net.minecraft.world.scores.ScoreAccess` |
| Server boss event | `ServerBossEvent` | `net.minecraft.server.level.ServerBossEvent` |
| Set objective packet | `ClientboundSetObjectivePacket` | `net.minecraft.network.protocol.game.ClientboundSetObjectivePacket` |
| Set score packet | `ClientboundSetScorePacket` | `net.minecraft.network.protocol.game.ClientboundSetScorePacket` |
| Display objective packet | `ClientboundSetDisplayObjectivePacket` | `net.minecraft.network.protocol.game.ClientboundSetDisplayObjectivePacket` |
| Player team packet | `ClientboundSetPlayerTeamPacket` | `net.minecraft.network.protocol.game.ClientboundSetPlayerTeamPacket` |
| Reset score packet | `ClientboundResetScorePacket` | `net.minecraft.network.protocol.game.ClientboundResetScorePacket` |
| Boss event packet | `ClientboundBossEventPacket` | `net.minecraft.network.protocol.game.ClientboundBossEventPacket` |

---

## Tasks

### 32.1 — `Objective` and criteria

```rust
// crates/oxidized-game/src/scoreboard/objective.rs

use serde::{Deserialize, Serialize};

/// How the score value is rendered in the client HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderType {
    Integer,
    Hearts,
}

/// Built-in objective criterion types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveCriterion {
    Dummy,
    Trigger,
    DeathCount,
    PlayerKillCount,
    TotalKillCount,
    /// Read-only: tracks entity health (0-20, halved for display).
    Health,
    Food,
    XpLevel,
    Armor,
    Custom(String),
}

impl ObjectiveCriterion {
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Health | Self::Food | Self::XpLevel | Self::Armor)
    }

    pub fn default_render_type(&self) -> RenderType {
        if matches!(self, Self::Health) { RenderType::Hearts } else { RenderType::Integer }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "dummy"            => Self::Dummy,
            "trigger"          => Self::Trigger,
            "deathCount"       => Self::DeathCount,
            "playerKillCount"  => Self::PlayerKillCount,
            "totalKillCount"   => Self::TotalKillCount,
            "health"           => Self::Health,
            "food"             => Self::Food,
            "xp"               => Self::XpLevel,
            "armor"            => Self::Armor,
            other              => Self::Custom(other.to_string()),
        }
    }
}

/// A scoreboard objective (column of scores).
#[derive(Debug, Clone)]
pub struct Objective {
    pub name:         String,
    pub display_name: Option<String>,    // JSON text component; None = use name
    pub criterion:    ObjectiveCriterion,
    pub render_type:  RenderType,
}

impl Objective {
    pub fn new(name: String, criterion: ObjectiveCriterion) -> Self {
        let render_type = criterion.default_render_type();
        Self { name, display_name: None, criterion, render_type }
    }
}
```

### 32.2 — Display slots

```rust
// crates/oxidized-game/src/scoreboard/display_slot.rs

/// All supported display slot identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DisplaySlot {
    List,       // tab list
    Sidebar,
    BelowName,
    // 16 team-colored sidebar slots
    SidebarTeamBlack,
    SidebarTeamDarkBlue,
    SidebarTeamDarkGreen,
    SidebarTeamDarkAqua,
    SidebarTeamDarkRed,
    SidebarTeamDarkPurple,
    SidebarTeamGold,
    SidebarTeamGray,
    SidebarTeamDarkGray,
    SidebarTeamBlue,
    SidebarTeamGreen,
    SidebarTeamAqua,
    SidebarTeamRed,
    SidebarTeamLightPurple,
    SidebarTeamYellow,
    SidebarTeamWhite,
}

impl DisplaySlot {
    /// Wire slot index matching Java's `DisplaySlot.id`.
    pub fn slot_id(self) -> u8 {
        match self {
            Self::List          => 0,
            Self::Sidebar       => 1,
            Self::BelowName     => 2,
            Self::SidebarTeamBlack      => 3,
            Self::SidebarTeamDarkBlue   => 4,
            Self::SidebarTeamDarkGreen  => 5,
            Self::SidebarTeamDarkAqua   => 6,
            Self::SidebarTeamDarkRed    => 7,
            Self::SidebarTeamDarkPurple => 8,
            Self::SidebarTeamGold       => 9,
            Self::SidebarTeamGray       => 10,
            Self::SidebarTeamDarkGray   => 11,
            Self::SidebarTeamBlue       => 12,
            Self::SidebarTeamGreen      => 13,
            Self::SidebarTeamAqua       => 14,
            Self::SidebarTeamRed        => 15,
            Self::SidebarTeamLightPurple=> 16,
            Self::SidebarTeamYellow     => 17,
            Self::SidebarTeamWhite      => 18,
        }
    }

    pub fn from_slot_id(id: u8) -> Option<Self> {
        match id {
            0  => Some(Self::List),
            1  => Some(Self::Sidebar),
            2  => Some(Self::BelowName),
            3  => Some(Self::SidebarTeamBlack),
            _  => None,
        }
    }
}
```

### 32.3 — `PlayerScore`

```rust
// crates/oxidized-game/src/scoreboard/score.rs

/// A single player's score for one objective.
#[derive(Debug, Clone)]
pub struct PlayerScore {
    pub owner:         String,  // player name or entity UUID string
    pub objective:     String,  // objective name
    pub value:         i32,
    /// Custom display override (None = show numeric value).
    pub display:       Option<String>, // JSON text component
    /// Number format override (None = default for objective's render type).
    pub number_format: Option<NumberFormat>,
    /// Whether this score has been locked (trigger objectives need explicit unlock).
    pub locked:        bool,
}

#[derive(Debug, Clone)]
pub enum NumberFormat {
    Blank,
    Styled { color: u32, decorations: u8 },
    Fixed(String), // JSON text component
}

impl PlayerScore {
    pub fn new(owner: String, objective: String, value: i32) -> Self {
        Self { owner, objective, value, display: None, number_format: None, locked: false }
    }
}
```

### 32.4 — `PlayerTeam`

```rust
// crates/oxidized-game/src/scoreboard/team.rs

/// Team collision rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionRule {
    Always,
    PushOtherTeams,
    PushOwnTeam,
    Never,
}

impl CollisionRule {
    pub fn id(self) -> &'static str {
        match self {
            Self::Always         => "always",
            Self::PushOtherTeams => "pushOtherTeams",
            Self::PushOwnTeam    => "pushOwnTeam",
            Self::Never          => "never",
        }
    }
}

/// Team name visibility rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NametagVisibility {
    Always,
    HideForOtherTeams,
    HideForOwnTeam,
    Never,
}

impl NametagVisibility {
    pub fn id(self) -> &'static str {
        match self {
            Self::Always             => "always",
            Self::HideForOtherTeams  => "hideForOtherTeams",
            Self::HideForOwnTeam     => "hideForOwnTeam",
            Self::Never              => "never",
        }
    }
}

/// Chat formatting color codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFormatting {
    Black = 0, DarkBlue, DarkGreen, DarkAqua, DarkRed, DarkPurple,
    Gold, Gray, DarkGray, Blue, Green, Aqua, Red, LightPurple, Yellow, White,
    Reset = 21,
}

/// A scoreboard team.
#[derive(Debug, Clone)]
pub struct PlayerTeam {
    pub name:                  String,
    pub display_name:          Option<String>,   // JSON text component
    pub color:                 ChatFormatting,
    pub prefix:                Option<String>,   // JSON text component
    pub suffix:                Option<String>,   // JSON text component
    pub collision_rule:        CollisionRule,
    pub nametag_visibility:    NametagVisibility,
    pub death_message_visibility: NametagVisibility,
    pub allow_friendly_fire:   bool,
    pub see_friendly_invisibles: bool,
    pub members:               std::collections::HashSet<String>,
}

impl PlayerTeam {
    pub fn new(name: String) -> Self {
        Self {
            name,
            display_name: None,
            color: ChatFormatting::Reset,
            prefix: None,
            suffix: None,
            collision_rule: CollisionRule::Always,
            nametag_visibility: NametagVisibility::Always,
            death_message_visibility: NametagVisibility::Always,
            allow_friendly_fire: true,
            see_friendly_invisibles: true,
            members: std::collections::HashSet::new(),
        }
    }
}
```

### 32.5 — `Scoreboard`

```rust
// crates/oxidized-game/src/scoreboard/scoreboard.rs

use std::collections::HashMap;
use super::{Objective, PlayerScore, PlayerTeam, DisplaySlot};

pub struct Scoreboard {
    pub objectives:  HashMap<String, Objective>,
    /// scores[(owner, objective)] = score
    pub scores:      HashMap<(String, String), PlayerScore>,
    pub teams:       HashMap<String, PlayerTeam>,
    /// player_team[player_name] = team_name
    pub player_team: HashMap<String, String>,
    pub display:     [Option<String>; 19], // indexed by DisplaySlot::slot_id()
}

impl Scoreboard {
    pub fn new() -> Self {
        Self {
            objectives: HashMap::new(),
            scores: HashMap::new(),
            teams: HashMap::new(),
            player_team: HashMap::new(),
            display: std::array::from_fn(|_| None),
        }
    }

    pub fn add_objective(&mut self, obj: Objective) -> bool {
        if self.objectives.contains_key(&obj.name) { return false; }
        self.objectives.insert(obj.name.clone(), obj);
        true
    }

    pub fn remove_objective(&mut self, name: &str) {
        self.objectives.remove(name);
        self.scores.retain(|(_, obj), _| obj != name);
        for slot in self.display.iter_mut() {
            if slot.as_deref() == Some(name) { *slot = None; }
        }
    }

    pub fn set_display_slot(&mut self, slot: DisplaySlot, objective: Option<String>) {
        self.display[slot.slot_id() as usize] = objective;
    }

    pub fn get_score_mut(&mut self, owner: String, objective: String) -> &mut PlayerScore {
        self.scores.entry((owner.clone(), objective.clone()))
            .or_insert_with(|| PlayerScore::new(owner, objective, 0))
    }

    pub fn get_score(&self, owner: &str, objective: &str) -> Option<&PlayerScore> {
        self.scores.get(&(owner.to_string(), objective.to_string()))
    }

    pub fn reset_score(&mut self, owner: &str, objective: Option<&str>) {
        match objective {
            Some(obj) => { self.scores.remove(&(owner.to_string(), obj.to_string())); }
            None      => { self.scores.retain(|(o, _), _| o != owner); }
        }
    }

    pub fn add_team(&mut self, team: PlayerTeam) -> bool {
        if self.teams.contains_key(&team.name) { return false; }
        self.teams.insert(team.name.clone(), team);
        true
    }

    pub fn remove_team(&mut self, name: &str) {
        if let Some(team) = self.teams.remove(name) {
            for member in &team.members {
                self.player_team.remove(member);
            }
        }
    }

    pub fn add_player_to_team(&mut self, player: &str, team_name: &str) -> bool {
        // Remove from current team first
        if let Some(old_team) = self.player_team.remove(player) {
            if let Some(t) = self.teams.get_mut(&old_team) {
                t.members.remove(player);
            }
        }
        if let Some(team) = self.teams.get_mut(team_name) {
            team.members.insert(player.to_string());
            self.player_team.insert(player.to_string(), team_name.to_string());
            true
        } else {
            false
        }
    }

    pub fn remove_player_from_team(&mut self, player: &str) -> bool {
        if let Some(team_name) = self.player_team.remove(player) {
            if let Some(team) = self.teams.get_mut(&team_name) {
                team.members.remove(player);
            }
            true
        } else {
            false
        }
    }
}
```

### 32.6 — Scoreboard wire packets

```rust
// crates/oxidized-game/src/scoreboard/packets.rs

/// `ClientboundSetObjectivePacket` (0x5C)
#[derive(Debug, Clone)]
pub struct ClientboundSetObjectivePacket {
    pub name:        String,
    pub action:      ObjectiveAction,
    /// Required for Add and Update actions.
    pub display_name: Option<String>,
    pub render_type:  Option<RenderType>,
    pub number_format: Option<NumberFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveAction {
    Add    = 0,
    Remove = 1,
    Update = 2,
}

/// `ClientboundSetScorePacket` (0x5E)
#[derive(Debug, Clone)]
pub struct ClientboundSetScorePacket {
    /// Entity name or player name.
    pub owner:         String,
    pub objective:     String,
    pub score:         i32,
    pub display:       Option<String>,
    pub number_format: Option<NumberFormat>,
}

/// `ClientboundSetDisplayObjectivePacket` (0x5D)
#[derive(Debug, Clone)]
pub struct ClientboundSetDisplayObjectivePacket {
    pub slot:           DisplaySlot,
    /// Empty string removes the current objective from the slot.
    pub objective_name: String,
}

/// `ClientboundSetPlayerTeamPacket` (0x60)
#[derive(Debug, Clone)]
pub struct ClientboundSetPlayerTeamPacket {
    pub team_name: String,
    pub method:    TeamMethod,
}

#[derive(Debug, Clone)]
pub enum TeamMethod {
    /// Action 0: Create the team.
    Add(TeamParameters),
    /// Action 1: Remove the team.
    Remove,
    /// Action 2: Update team parameters.
    Update(TeamParameters),
    /// Action 3: Add players to the team.
    AddPlayers(Vec<String>),
    /// Action 4: Remove players from the team.
    RemovePlayers(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct TeamParameters {
    pub display_name:             String,
    pub friendly_flags:           u8,  // bit 0=allow_friendly_fire, bit 1=see_friendly_invisibles
    pub nametag_visibility:       String,
    pub collision_rule:           String,
    pub color:                    i32, // ChatFormatting ordinal
    pub prefix:                   String,
    pub suffix:                   String,
}

/// `ClientboundResetScorePacket` (0x43)
#[derive(Debug, Clone)]
pub struct ClientboundResetScorePacket {
    pub owner:     String,
    /// None = remove all scores for this owner.
    pub objective: Option<String>,
}
```

### 32.7 — `ServerBossEvent`

```rust
// crates/oxidized-game/src/scoreboard/boss_event.rs

use uuid::Uuid;
use std::collections::HashSet;

/// Color of the boss health bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossBarColor { Pink, Blue, Red, Green, Yellow, Purple, White }

impl BossBarColor {
    pub fn id(self) -> u8 {
        match self {
            Self::Pink   => 0, Self::Blue  => 1, Self::Red    => 2,
            Self::Green  => 3, Self::Yellow=> 4, Self::Purple => 5, Self::White => 6,
        }
    }
}

/// Notch-style segments overlay on the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossBarOverlay { Progress, Notched6, Notched10, Notched12, Notched20 }

impl BossBarOverlay {
    pub fn id(self) -> u8 {
        match self {
            Self::Progress  => 0, Self::Notched6 => 1, Self::Notched10 => 2,
            Self::Notched12 => 3, Self::Notched20 => 4,
        }
    }
}

/// A server-side boss bar tracked by the server and sent to subscribed players.
pub struct ServerBossEvent {
    pub id:               Uuid,
    pub name:             String,   // JSON text component
    pub progress:         f32,      // 0.0-1.0
    pub color:            BossBarColor,
    pub overlay:          BossBarOverlay,
    pub darken_screen:    bool,
    pub play_boss_music:  bool,
    pub create_world_fog: bool,
    /// Players currently subscribed to this bar.
    pub subscribed_players: HashSet<Uuid>,
}

impl ServerBossEvent {
    pub fn new(name: String, color: BossBarColor, overlay: BossBarOverlay) -> Self {
        Self {
            id:               Uuid::new_v4(),
            name, progress: 1.0, color, overlay,
            darken_screen: false, play_boss_music: false, create_world_fog: false,
            subscribed_players: HashSet::new(),
        }
    }

    /// Build the ADD operation packet for a new subscriber.
    pub fn add_packet(&self) -> ClientboundBossEventPacket {
        ClientboundBossEventPacket {
            id:        self.id,
            operation: BossEventOperation::Add {
                name:             self.name.clone(),
                progress:         self.progress,
                color:            self.color,
                overlay:          self.overlay,
                darken_screen:    self.darken_screen,
                play_boss_music:  self.play_boss_music,
                create_world_fog: self.create_world_fog,
            },
        }
    }

    pub fn remove_packet(&self) -> ClientboundBossEventPacket {
        ClientboundBossEventPacket { id: self.id, operation: BossEventOperation::Remove }
    }

    pub fn update_progress_packet(&self) -> ClientboundBossEventPacket {
        ClientboundBossEventPacket { id: self.id, operation: BossEventOperation::UpdatePct(self.progress) }
    }

    pub fn update_name_packet(&self) -> ClientboundBossEventPacket {
        ClientboundBossEventPacket { id: self.id, operation: BossEventOperation::UpdateName(self.name.clone()) }
    }
}

/// `ClientboundBossEventPacket` (0x0A)
#[derive(Debug, Clone)]
pub struct ClientboundBossEventPacket {
    pub id:        Uuid,
    pub operation: BossEventOperation,
}

#[derive(Debug, Clone)]
pub enum BossEventOperation {
    Add {
        name:             String,
        progress:         f32,
        color:            BossBarColor,
        overlay:          BossBarOverlay,
        darken_screen:    bool,
        play_boss_music:  bool,
        create_world_fog: bool,
    },
    Remove,
    UpdatePct(f32),
    UpdateName(String),
    UpdateStyle { color: BossBarColor, overlay: BossBarOverlay },
    UpdateFlags { darken_screen: bool, play_boss_music: bool, create_world_fog: bool },
}
```

### 32.8 — Scoreboard NBT persistence

```rust
// crates/oxidized-game/src/scoreboard/persistence.rs

use oxidized_nbt::NbtCompound;
use super::Scoreboard;

/// Save scoreboard state to NBT for `data/scoreboard.dat`.
pub fn save_scoreboard(sb: &Scoreboard) -> NbtCompound {
    let mut root = NbtCompound::new();

    // Objectives list
    let objectives: Vec<NbtCompound> = sb.objectives.values().map(|obj| {
        let mut t = NbtCompound::new();
        t.put_string("Name",        &obj.name);
        t.put_string("CriteriaName", &format!("{:?}", obj.criterion).to_lowercase());
        t.put_string("DisplayName", obj.display_name.as_deref().unwrap_or(&obj.name));
        t.put_string("RenderType",  if obj.render_type == RenderType::Hearts { "hearts" } else { "integer" });
        t
    }).collect();
    root.put_list("Objectives", objectives);

    // Scores list
    let scores: Vec<NbtCompound> = sb.scores.values().map(|score| {
        let mut t = NbtCompound::new();
        t.put_string("Name",      &score.owner);
        t.put_string("Objective", &score.objective);
        t.put_int("Score",        score.value);
        t.put_byte("Locked",      score.locked as i8);
        t
    }).collect();
    root.put_list("PlayerScores", scores);

    // Teams list
    let teams: Vec<NbtCompound> = sb.teams.values().map(|team| {
        let mut t = NbtCompound::new();
        t.put_string("Name",         &team.name);
        t.put_string("DisplayName",  team.display_name.as_deref().unwrap_or(&team.name));
        t.put_string("TeamColor",    &format!("{:?}", team.color).to_lowercase());
        t.put_string("CollisionRule", team.collision_rule.id());
        t.put_string("NameTagVisibility", team.nametag_visibility.id());
        t.put_byte("AllowFriendlyFire",    team.allow_friendly_fire as i8);
        t.put_byte("SeeFriendlyInvisibles",team.see_friendly_invisibles as i8);
        let members: Vec<NbtCompound> = team.members.iter().map(|m| {
            let mut mt = NbtCompound::new(); mt.put_string("text", m); mt
        }).collect();
        t.put_list("Players", members);
        t
    }).collect();
    root.put_list("Teams", teams);

    // Display slots
    let mut display_tag = NbtCompound::new();
    let slot_names = ["list", "sidebar", "belowName"];
    for (i, name) in slot_names.iter().enumerate() {
        if let Some(ref obj) = sb.display[i] {
            display_tag.put_string(name, obj);
        }
    }
    root.put_compound("DisplaySlots", display_tag);

    root
}

/// Load scoreboard state from NBT.
pub fn load_scoreboard(root: &NbtCompound) -> Scoreboard {
    let mut sb = Scoreboard::new();

    if let Some(objectives) = root.get_list("Objectives") {
        for t in objectives {
            let name   = t.get_string("Name").unwrap_or("").to_string();
            let crit   = ObjectiveCriterion::from_str(t.get_string("CriteriaName").unwrap_or("dummy"));
            let render = if t.get_string("RenderType") == Some("hearts") { RenderType::Hearts } else { RenderType::Integer };
            let mut obj = Objective::new(name, crit);
            obj.render_type = render;
            sb.add_objective(obj);
        }
    }

    if let Some(scores) = root.get_list("PlayerScores") {
        for t in scores {
            let owner = t.get_string("Name").unwrap_or("").to_string();
            let obj   = t.get_string("Objective").unwrap_or("").to_string();
            let val   = t.get_int("Score").unwrap_or(0);
            let locked = t.get_byte("Locked").unwrap_or(0) != 0;
            let score  = sb.get_score_mut(owner, obj);
            score.value  = val;
            score.locked = locked;
        }
    }

    if let Some(teams) = root.get_list("Teams") {
        for t in teams {
            let name = t.get_string("Name").unwrap_or("").to_string();
            let mut team = PlayerTeam::new(name.clone());
            team.display_name = t.get_string("DisplayName").map(String::from);
            team.allow_friendly_fire = t.get_byte("AllowFriendlyFire").unwrap_or(1) != 0;
            team.see_friendly_invisibles = t.get_byte("SeeFriendlyInvisibles").unwrap_or(1) != 0;
            if let Some(members) = t.get_list("Players") {
                for m in members {
                    if let Some(player) = m.get_string("text") {
                        team.members.insert(player.to_string());
                    }
                }
            }
            sb.add_team(team);
        }
    }

    sb
}
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::scoreboard

pub use objective::{Objective, ObjectiveCriterion, RenderType};
pub use display_slot::DisplaySlot;
pub use score::{PlayerScore, NumberFormat};
pub use team::{PlayerTeam, CollisionRule, NametagVisibility, ChatFormatting};
pub use scoreboard::Scoreboard;
pub use packets::{
    ClientboundSetObjectivePacket, ObjectiveAction,
    ClientboundSetScorePacket,
    ClientboundSetDisplayObjectivePacket,
    ClientboundSetPlayerTeamPacket, TeamMethod, TeamParameters,
    ClientboundResetScorePacket,
};
pub use boss_event::{
    ServerBossEvent, BossBarColor, BossBarOverlay,
    ClientboundBossEventPacket, BossEventOperation,
};
pub use persistence::{save_scoreboard, load_scoreboard};
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxidized_nbt::NbtCompound;

    // --- Objective ---

    #[test]
    fn health_criterion_defaults_to_hearts_render() {
        let obj = Objective::new("hp".to_string(), ObjectiveCriterion::Health);
        assert_eq!(obj.render_type, RenderType::Hearts);
    }

    #[test]
    fn dummy_criterion_defaults_to_integer_render() {
        let obj = Objective::new("kills".to_string(), ObjectiveCriterion::Dummy);
        assert_eq!(obj.render_type, RenderType::Integer);
    }

    #[test]
    fn health_criterion_is_read_only() {
        assert!(ObjectiveCriterion::Health.is_read_only());
        assert!(!ObjectiveCriterion::Dummy.is_read_only());
        assert!(!ObjectiveCriterion::DeathCount.is_read_only());
    }

    // --- Scoreboard add/remove objective ---

    #[test]
    fn add_objective_succeeds() {
        let mut sb = Scoreboard::new();
        let obj = Objective::new("kills".to_string(), ObjectiveCriterion::Dummy);
        assert!(sb.add_objective(obj));
    }

    #[test]
    fn add_duplicate_objective_fails() {
        let mut sb = Scoreboard::new();
        let obj = Objective::new("kills".to_string(), ObjectiveCriterion::Dummy);
        sb.add_objective(obj.clone());
        assert!(!sb.add_objective(obj));
    }

    #[test]
    fn remove_objective_clears_scores_and_display() {
        let mut sb = Scoreboard::new();
        let obj = Objective::new("kills".to_string(), ObjectiveCriterion::Dummy);
        sb.add_objective(obj);
        sb.get_score_mut("Alice".to_string(), "kills".to_string()).value = 5;
        sb.set_display_slot(DisplaySlot::Sidebar, Some("kills".to_string()));
        sb.remove_objective("kills");
        assert!(sb.objectives.is_empty());
        assert!(sb.scores.is_empty());
        assert!(sb.display[DisplaySlot::Sidebar.slot_id() as usize].is_none());
    }

    // --- PlayerScore ---

    #[test]
    fn score_starts_at_zero() {
        let mut sb = Scoreboard::new();
        let obj = Objective::new("test".to_string(), ObjectiveCriterion::Dummy);
        sb.add_objective(obj);
        let score = sb.get_score_mut("Bob".to_string(), "test".to_string());
        assert_eq!(score.value, 0);
    }

    #[test]
    fn reset_score_removes_single_objective() {
        let mut sb = Scoreboard::new();
        sb.add_objective(Objective::new("a".to_string(), ObjectiveCriterion::Dummy));
        sb.add_objective(Objective::new("b".to_string(), ObjectiveCriterion::Dummy));
        sb.get_score_mut("Alice".to_string(), "a".to_string()).value = 10;
        sb.get_score_mut("Alice".to_string(), "b".to_string()).value = 20;
        sb.reset_score("Alice", Some("a"));
        assert!(sb.get_score("Alice", "a").is_none());
        assert!(sb.get_score("Alice", "b").is_some());
    }

    #[test]
    fn reset_all_scores_for_player() {
        let mut sb = Scoreboard::new();
        sb.add_objective(Objective::new("a".to_string(), ObjectiveCriterion::Dummy));
        sb.add_objective(Objective::new("b".to_string(), ObjectiveCriterion::Dummy));
        sb.get_score_mut("Alice".to_string(), "a".to_string());
        sb.get_score_mut("Alice".to_string(), "b".to_string());
        sb.reset_score("Alice", None);
        assert!(sb.get_score("Alice", "a").is_none());
        assert!(sb.get_score("Alice", "b").is_none());
    }

    // --- Teams ---

    #[test]
    fn add_and_retrieve_team() {
        let mut sb = Scoreboard::new();
        let team = PlayerTeam::new("red".to_string());
        assert!(sb.add_team(team));
        assert!(sb.teams.contains_key("red"));
    }

    #[test]
    fn add_player_to_team() {
        let mut sb = Scoreboard::new();
        sb.add_team(PlayerTeam::new("blue".to_string()));
        assert!(sb.add_player_to_team("Alice", "blue"));
        assert_eq!(sb.player_team.get("Alice").map(String::as_str), Some("blue"));
        assert!(sb.teams["blue"].members.contains("Alice"));
    }

    #[test]
    fn player_moved_between_teams() {
        let mut sb = Scoreboard::new();
        sb.add_team(PlayerTeam::new("red".to_string()));
        sb.add_team(PlayerTeam::new("blue".to_string()));
        sb.add_player_to_team("Alice", "red");
        sb.add_player_to_team("Alice", "blue");
        assert!(!sb.teams["red"].members.contains("Alice"));
        assert!(sb.teams["blue"].members.contains("Alice"));
    }

    #[test]
    fn remove_player_from_team() {
        let mut sb = Scoreboard::new();
        sb.add_team(PlayerTeam::new("green".to_string()));
        sb.add_player_to_team("Bob", "green");
        assert!(sb.remove_player_from_team("Bob"));
        assert!(sb.player_team.get("Bob").is_none());
        assert!(!sb.teams["green"].members.contains("Bob"));
    }

    // --- DisplaySlot ---

    #[test]
    fn display_slot_ids_are_sequential() {
        assert_eq!(DisplaySlot::List.slot_id(),      0);
        assert_eq!(DisplaySlot::Sidebar.slot_id(),   1);
        assert_eq!(DisplaySlot::BelowName.slot_id(), 2);
    }

    // --- BossBarColor and Overlay ---

    #[test]
    fn boss_bar_color_ids_are_unique() {
        use std::collections::HashSet;
        let ids: HashSet<u8> = [
            BossBarColor::Pink, BossBarColor::Blue, BossBarColor::Red,
            BossBarColor::Green, BossBarColor::Yellow, BossBarColor::Purple, BossBarColor::White,
        ].iter().map(|c| c.id()).collect();
        assert_eq!(ids.len(), 7);
    }

    #[test]
    fn boss_event_add_packet_preserves_fields() {
        let bar = ServerBossEvent::new(
            "{\"text\":\"Ender Dragon\"}".to_string(),
            BossBarColor::Purple,
            BossBarOverlay::Progress,
        );
        let pkt = bar.add_packet();
        assert_eq!(pkt.id, bar.id);
        match pkt.operation {
            BossEventOperation::Add { color, overlay, .. } => {
                assert_eq!(color,   BossBarColor::Purple);
                assert_eq!(overlay, BossBarOverlay::Progress);
            }
            _ => panic!("expected Add operation"),
        }
    }

    // --- NBT persistence ---

    #[test]
    fn scoreboard_nbt_roundtrip_objectives() {
        let mut sb = Scoreboard::new();
        sb.add_objective(Objective::new("kills".to_string(), ObjectiveCriterion::Dummy));
        sb.get_score_mut("Alice".to_string(), "kills".to_string()).value = 42;
        let nbt  = save_scoreboard(&sb);
        let sb2  = load_scoreboard(&nbt);
        assert!(sb2.objectives.contains_key("kills"));
        assert_eq!(sb2.get_score("Alice", "kills").map(|s| s.value), Some(42));
    }

    #[test]
    fn scoreboard_nbt_roundtrip_teams() {
        let mut sb = Scoreboard::new();
        let mut team = PlayerTeam::new("red".to_string());
        team.members.insert("Alice".to_string());
        sb.add_team(team);
        let nbt = save_scoreboard(&sb);
        let sb2 = load_scoreboard(&nbt);
        assert!(sb2.teams.contains_key("red"));
        assert!(sb2.teams["red"].members.contains("Alice"));
    }

    // --- CollisionRule and NametagVisibility ---

    #[test]
    fn collision_rule_ids_match_vanilla() {
        assert_eq!(CollisionRule::Always.id(),         "always");
        assert_eq!(CollisionRule::PushOtherTeams.id(), "pushOtherTeams");
        assert_eq!(CollisionRule::Never.id(),          "never");
    }

    #[test]
    fn nametag_visibility_ids_match_vanilla() {
        assert_eq!(NametagVisibility::Always.id(),            "always");
        assert_eq!(NametagVisibility::HideForOtherTeams.id(), "hideForOtherTeams");
    }
}
```
