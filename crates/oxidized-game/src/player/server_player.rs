//! Runtime player state.
//!
//! `ServerPlayer` holds the authoritative game state for a connected player:
//! position, rotation, health, game mode, abilities, teleport tracking,
//! dimension, and spawn point.
//!
//! Mirrors `net.minecraft.server.level.ServerPlayer`.

use std::collections::VecDeque;
use std::time::Instant;

use oxidized_nbt::{NbtCompound, NbtList, NbtTag, TAG_COMPOUND, TAG_DOUBLE, TAG_FLOAT};
use oxidized_protocol::auth::GameProfile;
use oxidized_protocol::types::{BlockPos, ResourceLocation, Vec3};
use uuid::Uuid;

use super::abilities::PlayerAbilities;
use super::game_mode::GameMode;
use super::inventory::PlayerInventory;
use crate::inventory::ItemStack;

// ---------------------------------------------------------------------------
// Sub-structs — logical groupings extracted from ServerPlayer (R5.11).
// ---------------------------------------------------------------------------

/// Player position, rotation, and movement flags.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerMovement {
    /// World position (double precision, matches protocol).
    pub pos: Vec3,
    /// Yaw rotation in degrees (horizontal).
    pub yaw: f32,
    /// Pitch rotation in degrees (vertical).
    pub pitch: f32,
    /// Whether the player is on the ground.
    pub is_on_ground: bool,
    /// Whether the player is currently sneaking (shift held).
    pub is_sneaking: bool,
    /// Whether the player is currently sprinting.
    pub is_sprinting: bool,
    /// Whether the player is currently fall-flying (elytra glide).
    pub is_fall_flying: bool,
}

/// Health, food, score, and absorption state.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatStats {
    /// Current health (0.0–max_health).
    pub health: f32,
    /// Maximum health (default 20.0).
    pub max_health: f32,
    /// Current food level (0–20).
    pub food_level: i32,
    /// Food saturation level.
    pub food_saturation: f32,
    /// Player's score (incremented by XP orbs, reset on death).
    pub score: i32,
    /// Absorption hearts from golden apples / effects (0.0+).
    pub absorption_amount: f32,
    /// Location of last death: `(dimension, packed BlockPos)`.
    pub last_death_location: Option<(ResourceLocation, i64)>,
}

/// Experience levels and enchanting seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerExperience {
    /// Experience level (0+).
    pub xp_level: i32,
    /// Experience bar progress within the current level (0.0–1.0).
    pub xp_progress: f32,
    /// Total experience points earned.
    pub xp_total: i32,
    /// Seed for enchanting table randomization.
    pub xp_seed: i32,
}

/// Personal spawn point and current dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnInfo {
    /// Current dimension the player is in.
    pub dimension: ResourceLocation,
    /// Personal spawn point (bed/respawn anchor).
    pub spawn_pos: BlockPos,
    /// Spawn yaw angle.
    pub spawn_angle: f32,
}

/// Client connection parameters and rate limiting.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Chunk render distance (in chunks).
    pub view_distance: i32,
    /// Entity simulation distance (in chunks).
    pub simulation_distance: i32,
    /// Client-requested chunk receive rate (chunks per tick).
    pub chunk_send_rate: f32,
    /// Smoothed round-trip latency in milliseconds (exponential moving average).
    pub latency: i32,
    /// Displayed skin parts bitmask (cape, jacket, sleeves, pants, hat).
    pub model_customisation: u8,
    /// Movement packet rate limiter: (count this second, second start instant).
    pub movement_rate: (u32, Instant),
}

/// Teleport confirmation tracking.
#[derive(Debug, Clone)]
pub struct TeleportTracker {
    /// Pending teleports the client has not yet confirmed (ID, target position, sent time).
    pub pending: VecDeque<(i32, Vec3, Instant)>,
    /// Next teleport ID to assign.
    id_counter: i32,
}

impl TeleportTracker {
    /// Creates a new empty tracker.
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            id_counter: 0,
        }
    }

    /// Returns the next teleport ID (monotonically increasing, wrapping).
    pub fn next_id(&mut self) -> i32 {
        self.id_counter = self.id_counter.wrapping_add(1);
        self.id_counter
    }
}

/// Block mining in-progress state.
#[derive(Debug, Clone, Copy)]
pub struct MiningState {
    /// Position where survival block mining started.
    pub start_pos: Option<BlockPos>,
    /// Instant when survival block mining started.
    pub start_time: Option<Instant>,
}

/// Raw NBT preserved for unimplemented systems (roundtripped to prevent data loss).
#[derive(Debug, Clone, Default)]
pub struct RawPlayerNbt {
    /// Active potion/status effects (raw NBT, preserved until effect system is built).
    pub active_effects: Option<NbtTag>,
    /// Attribute modifiers (raw NBT, preserved until attribute system is built).
    pub attributes: Option<NbtTag>,
    /// Ender chest inventory (raw NBT, preserved until ender chest is implemented).
    pub ender_items: Option<NbtTag>,
}

/// Runtime state for a single connected player.
///
/// # Examples
///
/// ```
/// use oxidized_game::player::ServerPlayer;
/// use oxidized_game::player::GameMode;
/// use oxidized_protocol::auth::GameProfile;
/// use oxidized_protocol::types::ResourceLocation;
/// use uuid::Uuid;
///
/// let uuid = Uuid::nil();
/// let profile = GameProfile::new(uuid, "Steve".into());
/// let player = ServerPlayer::new(
///     1,
///     profile,
///     ResourceLocation::minecraft("overworld"),
///     GameMode::Survival,
/// );
/// assert_eq!(player.name, "Steve");
/// assert_eq!(player.entity_id, 1);
/// ```
#[derive(Debug)]
pub struct ServerPlayer {
    // -- Identity (kept at top level — accessed everywhere) --
    /// Network entity ID (unique per server session, assigned by [`PlayerList`](crate::player::PlayerList)).
    pub entity_id: i32,
    /// Mojang account UUID.
    pub uuid: Uuid,
    /// Display name.
    pub name: String,
    /// Full Mojang profile (UUID, name, skin textures).
    pub profile: GameProfile,

    // -- Position / rotation / movement flags --
    /// Movement state (position, rotation, on-ground, sneak, sprint, elytra).
    pub movement: PlayerMovement,

    // -- Game state --
    /// Current game mode.
    pub game_mode: GameMode,
    /// Previous game mode (for respawn packets). `None` = no previous.
    pub previous_game_mode: Option<GameMode>,
    /// Current abilities derived from game mode.
    pub abilities: PlayerAbilities,
    /// Player inventory (46 slots).
    pub inventory: PlayerInventory,

    // -- Grouped sub-structs --
    /// Health, food, score, absorption, and death tracking.
    pub combat: CombatStats,
    /// Experience levels, progress, and enchanting seed.
    pub experience: PlayerExperience,
    /// Current dimension and personal spawn point.
    pub spawn: SpawnInfo,
    /// Client connection settings (view distance, latency, skin parts, rate limit).
    pub connection: ConnectionInfo,
    /// Teleport confirmation tracking.
    pub teleport: TeleportTracker,
    /// Block mining in-progress state.
    pub mining: MiningState,
    /// Raw NBT for unimplemented systems.
    pub raw_nbt: RawPlayerNbt,
}

impl ServerPlayer {
    /// Creates a new player with the given entity ID, profile, dimension, and game mode.
    ///
    /// The entity ID should be obtained from [`PlayerList::next_entity_id`](crate::player::PlayerList::next_entity_id).
    ///
    /// Only 4 required parameters — a builder is not needed. If the
    /// constructor grows beyond 5 parameters, introduce a
    /// `ServerPlayerBuilder` at that point.
    pub fn new(
        entity_id: i32,
        profile: GameProfile,
        dimension: ResourceLocation,
        game_mode: GameMode,
    ) -> Self {
        let uuid = profile.uuid().unwrap_or(Uuid::nil());
        let name = profile.name().to_owned();
        let abilities = PlayerAbilities::for_game_mode(game_mode);
        Self {
            entity_id,
            uuid,
            name,
            profile,
            movement: PlayerMovement {
                pos: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                is_on_ground: false,
                is_sneaking: false,
                is_sprinting: false,
                is_fall_flying: false,
            },
            game_mode,
            previous_game_mode: None,
            abilities,
            inventory: PlayerInventory::new(),
            combat: CombatStats {
                health: 20.0,
                max_health: 20.0,
                food_level: 20,
                food_saturation: 5.0,
                score: 0,
                absorption_amount: 0.0,
                last_death_location: None,
            },
            experience: PlayerExperience {
                xp_level: 0,
                xp_progress: 0.0,
                xp_total: 0,
                xp_seed: 0,
            },
            spawn: SpawnInfo {
                dimension,
                spawn_pos: BlockPos::new(0, 64, 0),
                spawn_angle: 0.0,
            },
            connection: ConnectionInfo {
                view_distance: 10,
                simulation_distance: 10,
                chunk_send_rate: 25.0,
                latency: 0,
                model_customisation: 0xFF, // all parts visible by default
                movement_rate: (0, Instant::now()),
            },
            teleport: TeleportTracker::new(),
            mining: MiningState {
                start_pos: None,
                start_time: None,
            },
            raw_nbt: RawPlayerNbt::default(),
        }
    }

    /// Returns the chunk X coordinate containing this player.
    pub fn chunk_x(&self) -> i32 {
        (self.movement.pos.x as i32) >> 4
    }

    /// Returns the chunk Z coordinate containing this player.
    pub fn chunk_z(&self) -> i32 {
        (self.movement.pos.z as i32) >> 4
    }

    /// Loads player state from an NBT compound (`playerdata/<uuid>.dat`).
    ///
    /// Missing fields fall back to sensible defaults (matching vanilla behavior
    /// for corrupt or partial player data).
    pub fn load_from_nbt(&mut self, nbt: &NbtCompound) {
        // Position — TAG_List of 3 doubles
        if let Some(pos_list) = nbt.get_list("Pos") {
            let values: Vec<f64> = pos_list.iter().filter_map(|t| t.as_double()).collect();
            if values.len() == 3 {
                self.movement.pos = Vec3::new(values[0], values[1], values[2]);
            }
        }

        // Rotation — TAG_List of 2 floats
        if let Some(rot_list) = nbt.get_list("Rotation") {
            let values: Vec<f32> = rot_list.iter().filter_map(|t| t.as_float()).collect();
            if values.len() == 2 {
                self.movement.yaw = values[0];
                self.movement.pitch = values[1];
            }
        }

        // Game mode
        if let Some(v) = nbt.get_int("playerGameType") {
            self.game_mode = GameMode::from_id(v);
            self.abilities = PlayerAbilities::for_game_mode(self.game_mode);
        }

        if let Some(v) = nbt.get_int("previousPlayerGameType") {
            let mode = GameMode::from_id(v);
            self.previous_game_mode = Some(mode);
        }

        // Health / food
        if let Some(v) = nbt.get_float("Health") {
            self.combat.health = v.clamp(0.0, self.combat.max_health);
        }
        if let Some(v) = nbt.get_int("foodLevel") {
            self.combat.food_level = v.clamp(0, 20);
        }
        if let Some(v) = nbt.get_float("foodSaturationLevel") {
            self.combat.food_saturation = v.clamp(0.0, self.combat.food_level as f32);
        }

        // Spawn position (optional — bed or respawn anchor)
        if let (Some(sx), Some(sy), Some(sz)) = (
            nbt.get_int("SpawnX"),
            nbt.get_int("SpawnY"),
            nbt.get_int("SpawnZ"),
        ) {
            self.spawn.spawn_pos = BlockPos::new(sx, sy, sz);
        }
        if let Some(angle) = nbt.get_float("SpawnAngle") {
            self.spawn.spawn_angle = angle;
        }

        // Dimension
        if let Some(dim) = nbt.get_string("Dimension") {
            if let Ok(loc) = ResourceLocation::from_string(dim) {
                self.spawn.dimension = loc;
            }
        }

        self.movement.is_on_ground = nbt.get_byte("OnGround").unwrap_or(0) != 0;
        self.movement.is_sneaking = nbt.get_byte("Sneaking").unwrap_or(0) != 0;

        // Selected hotbar slot
        if let Some(v) = nbt.get_int("SelectedItemSlot") {
            if (0..9).contains(&v) {
                self.inventory.selected_slot = v as u8;
            }
        }

        // Inventory items
        if let Some(inv_list) = nbt.get_list("Inventory") {
            for tag in inv_list.iter() {
                if let NbtTag::Compound(item_nbt) = tag {
                    let slot = item_nbt.get_byte("Slot").unwrap_or(-1);
                    if slot < 0 {
                        continue;
                    }
                    let slot = slot as usize;
                    if slot >= PlayerInventory::TOTAL_SLOTS {
                        continue;
                    }
                    if let Ok(stack) = ItemStack::from_nbt(item_nbt) {
                        self.inventory.set(slot, stack);
                    }
                }
            }
        }

        // Experience
        self.experience.xp_level = nbt.get_int("XpLevel").unwrap_or(0);
        self.experience.xp_progress = nbt.get_float("XpP").unwrap_or(0.0).clamp(0.0, 1.0);
        self.experience.xp_total = nbt.get_int("XpTotal").unwrap_or(0);
        self.experience.xp_seed = nbt.get_int("XpSeed").unwrap_or(0);

        // Score
        self.combat.score = nbt.get_int("Score").unwrap_or(0);

        // Absorption
        self.combat.absorption_amount =
            nbt.get_float("AbsorptionAmount").unwrap_or(0.0).max(0.0);

        // Fall-flying (elytra glide state)
        self.movement.is_fall_flying = nbt.get_byte("FallFlying").unwrap_or(0) != 0;

        // Abilities — load full compound, falling back to game-mode defaults
        if let Some(ab) = nbt.get_compound("abilities") {
            self.abilities.is_invulnerable = ab.get_byte("is_invulnerable").unwrap_or(0) != 0;
            self.abilities.is_flying = ab.get_byte("is_flying").unwrap_or(0) != 0;
            self.abilities.can_fly = ab.get_byte("mayfly").unwrap_or(0) != 0;
            self.abilities.is_instabuild = ab.get_byte("is_instabuild").unwrap_or(0) != 0;
            if let Some(fs) = ab.get_float("flySpeed") {
                self.abilities.fly_speed = fs;
            }
            if let Some(ws) = ab.get_float("walkSpeed") {
                self.abilities.walk_speed = ws;
            }
        }

        // Last death location
        if let Some(death_compound) = nbt.get_compound("LastDeathLocation") {
            if let Some(dim_str) = death_compound.get_string("dimension") {
                if let Ok(dim) = ResourceLocation::from_string(dim_str) {
                    let pos = death_compound.get_long("pos").unwrap_or(0);
                    self.combat.last_death_location = Some((dim, pos));
                }
            }
        }

        // Roundtrip raw NBT for unimplemented systems
        if let Some(tag) = nbt.get("active_effects") {
            self.raw_nbt.active_effects = Some(tag.clone());
        }
        if let Some(tag) = nbt.get("attributes") {
            self.raw_nbt.attributes = Some(tag.clone());
        }
        if let Some(tag) = nbt.get("EnderItems") {
            self.raw_nbt.ender_items = Some(tag.clone());
        }
    }

    /// Saves player state to an NBT compound for disk persistence.
    pub fn save_to_nbt(&self) -> NbtCompound {
        let mut nbt = NbtCompound::new();

        // Position
        let mut pos = NbtList::new(TAG_DOUBLE);
        let _ = pos.push(NbtTag::Double(self.movement.pos.x));
        let _ = pos.push(NbtTag::Double(self.movement.pos.y));
        let _ = pos.push(NbtTag::Double(self.movement.pos.z));
        nbt.put("Pos", NbtTag::List(pos));

        // Rotation
        let mut rot = NbtList::new(TAG_FLOAT);
        let _ = rot.push(NbtTag::Float(self.movement.yaw));
        let _ = rot.push(NbtTag::Float(self.movement.pitch));
        nbt.put("Rotation", NbtTag::List(rot));

        nbt.put_int("playerGameType", self.game_mode.id());
        if let Some(prev) = self.previous_game_mode {
            nbt.put_int("previousPlayerGameType", prev.id());
        }
        nbt.put_float("Health", self.combat.health);
        nbt.put_int("foodLevel", self.combat.food_level);
        nbt.put_float("foodSaturationLevel", self.combat.food_saturation);
        nbt.put_byte("OnGround", u8::from(self.movement.is_on_ground) as i8);
        nbt.put_byte("Sneaking", u8::from(self.movement.is_sneaking) as i8);

        // Spawn position
        nbt.put_int("SpawnX", self.spawn.spawn_pos.x);
        nbt.put_int("SpawnY", self.spawn.spawn_pos.y);
        nbt.put_int("SpawnZ", self.spawn.spawn_pos.z);
        nbt.put_float("SpawnAngle", self.spawn.spawn_angle);

        // Dimension
        nbt.put_string("Dimension", self.spawn.dimension.to_string());

        // Selected hotbar slot
        nbt.put_int("SelectedItemSlot", self.inventory.selected_slot as i32);

        // Inventory items
        let mut inv_list = NbtList::new(TAG_COMPOUND);
        for slot in 0..PlayerInventory::TOTAL_SLOTS {
            let stack = self.inventory.get(slot);
            if !stack.is_empty() {
                if let Some(mut item_nbt) = stack.to_nbt() {
                    item_nbt.put_byte("Slot", slot as i8);
                    let _ = inv_list.push(NbtTag::Compound(item_nbt));
                }
            }
        }
        nbt.put("Inventory", NbtTag::List(inv_list));

        // Experience
        nbt.put_int("XpLevel", self.experience.xp_level);
        nbt.put_float("XpP", self.experience.xp_progress);
        nbt.put_int("XpTotal", self.experience.xp_total);
        nbt.put_int("XpSeed", self.experience.xp_seed);

        // Score
        nbt.put_int("Score", self.combat.score);

        // Absorption
        nbt.put_float("AbsorptionAmount", self.combat.absorption_amount);

        // Fall-flying
        nbt.put_byte("FallFlying", i8::from(self.movement.is_fall_flying));

        // Abilities compound
        let mut ab = NbtCompound::new();
        ab.put_byte("is_invulnerable", i8::from(self.abilities.is_invulnerable));
        ab.put_byte("is_flying", i8::from(self.abilities.is_flying));
        ab.put_byte("mayfly", i8::from(self.abilities.can_fly));
        ab.put_byte("is_instabuild", i8::from(self.abilities.is_instabuild));
        ab.put_float("flySpeed", self.abilities.fly_speed);
        ab.put_float("walkSpeed", self.abilities.walk_speed);
        nbt.put("abilities", NbtTag::Compound(ab));

        // Last death location
        if let Some((ref dim, pos)) = self.combat.last_death_location {
            let mut death = NbtCompound::new();
            death.put_string("dimension", dim.to_string());
            death.put_long("pos", pos);
            nbt.put("LastDeathLocation", NbtTag::Compound(death));
        }

        // Roundtrip raw NBT for unimplemented systems
        if let Some(ref tag) = self.raw_nbt.active_effects {
            nbt.put("active_effects", tag.clone());
        }
        if let Some(ref tag) = self.raw_nbt.attributes {
            nbt.put("attributes", tag.clone());
        }
        if let Some(ref tag) = self.raw_nbt.ender_items {
            nbt.put("EnderItems", tag.clone());
        }

        nbt
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn make_test_player(id: i32, name: &str) -> ServerPlayer {
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, name.into());
        ServerPlayer::new(
            id,
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        )
    }

    #[test]
    fn test_new_player() {
        let uuid = Uuid::new_v4();
        let profile = GameProfile::new(uuid, "Steve".into());
        let player = ServerPlayer::new(
            1,
            profile,
            ResourceLocation::minecraft("overworld"),
            GameMode::Survival,
        );

        assert_eq!(player.entity_id, 1);
        assert_eq!(player.uuid, uuid);
        assert_eq!(player.name, "Steve");
        assert_eq!(player.movement.pos, Vec3::ZERO);
        assert_eq!(player.game_mode, GameMode::Survival);
        assert_eq!(player.previous_game_mode, None);
        assert!((player.combat.health - 20.0).abs() < f32::EPSILON);
        assert!((player.combat.max_health - 20.0).abs() < f32::EPSILON);
        assert_eq!(player.combat.food_level, 20);
        assert_eq!(player.connection.view_distance, 10);
        assert_eq!(player.connection.simulation_distance, 10);
        assert!(player.teleport.pending.is_empty());
    }

    #[test]
    fn test_teleport_id_increments() {
        let mut player = make_test_player(1, "Test");
        assert_eq!(player.teleport.next_id(), 1);
        assert_eq!(player.teleport.next_id(), 2);
        assert_eq!(player.teleport.next_id(), 3);
    }

    #[test]
    fn test_chunk_coordinates() {
        let mut player = make_test_player(1, "Test");
        player.movement.pos = Vec3::new(100.0, 64.0, -200.0);
        assert_eq!(player.chunk_x(), 6); // 100 >> 4 = 6
        assert_eq!(player.chunk_z(), -13); // -200 >> 4 = -13
    }

    #[test]
    fn test_chunk_coordinates_origin() {
        let player = make_test_player(1, "Test");
        assert_eq!(player.chunk_x(), 0);
        assert_eq!(player.chunk_z(), 0);
    }

    #[test]
    fn test_load_from_nbt() {
        let mut nbt = NbtCompound::new();

        let mut pos = NbtList::new(TAG_DOUBLE);
        let _ = pos.push(NbtTag::Double(50.5));
        let _ = pos.push(NbtTag::Double(70.0));
        let _ = pos.push(NbtTag::Double(-100.25));
        nbt.put("Pos", NbtTag::List(pos));

        let mut rot = NbtList::new(TAG_FLOAT);
        let _ = rot.push(NbtTag::Float(90.0));
        let _ = rot.push(NbtTag::Float(-15.0));
        nbt.put("Rotation", NbtTag::List(rot));

        nbt.put_int("playerGameType", 1);
        nbt.put_int("previousPlayerGameType", 0);
        nbt.put_float("Health", 15.0);
        nbt.put_int("foodLevel", 18);
        nbt.put_float("foodSaturationLevel", 3.5);
        nbt.put_byte("OnGround", 1);
        nbt.put_int("SpawnX", 10);
        nbt.put_int("SpawnY", 65);
        nbt.put_int("SpawnZ", -20);
        nbt.put_string("Dimension", "minecraft:the_nether");

        let mut player = make_test_player(1, "Test");
        player.load_from_nbt(&nbt);

        assert!((player.movement.pos.x - 50.5).abs() < 0.001);
        assert!((player.movement.pos.y - 70.0).abs() < 0.001);
        assert!((player.movement.pos.z + 100.25).abs() < 0.001);
        assert!((player.movement.yaw - 90.0).abs() < f32::EPSILON);
        assert!((player.movement.pitch + 15.0).abs() < f32::EPSILON);
        assert_eq!(player.game_mode, GameMode::Creative);
        assert_eq!(player.previous_game_mode, Some(GameMode::Survival));
        assert!((player.combat.health - 15.0).abs() < f32::EPSILON);
        assert_eq!(player.combat.food_level, 18);
        assert!((player.combat.food_saturation - 3.5).abs() < f32::EPSILON);
        assert!(player.movement.is_on_ground);
        assert_eq!(player.spawn.spawn_pos, BlockPos::new(10, 65, -20));
    }

    #[test]
    fn test_load_from_empty_nbt_keeps_defaults() {
        let nbt = NbtCompound::new();
        let mut player = make_test_player(1, "Test");
        player.movement.pos = Vec3::new(10.0, 64.0, 20.0);
        player.load_from_nbt(&nbt);

        // Position should remain unchanged with empty NBT.
        assert!((player.movement.pos.x - 10.0).abs() < 0.001);
        assert!((player.movement.pos.y - 64.0).abs() < 0.001);
        assert_eq!(player.game_mode, GameMode::Survival);
    }

    #[test]
    fn test_save_to_nbt_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player.movement.pos = Vec3::new(50.5, 70.0, -100.25);
        player.movement.yaw = 90.0;
        player.movement.pitch = -15.0;
        player.game_mode = GameMode::Creative;
        player.previous_game_mode = Some(GameMode::Survival);
        player.combat.health = 15.0;
        player.combat.food_level = 18;
        player.movement.is_on_ground = true;
        player.spawn.spawn_pos = BlockPos::new(10, 65, -20);

        let nbt = player.save_to_nbt();

        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert!((player2.movement.pos.x - 50.5).abs() < 0.001);
        assert!((player2.movement.pos.y - 70.0).abs() < 0.001);
        assert_eq!(player2.game_mode, GameMode::Creative);
        assert_eq!(player2.previous_game_mode, Some(GameMode::Survival));
        assert!((player2.combat.health - 15.0).abs() < f32::EPSILON);
        assert_eq!(player2.combat.food_level, 18);
        assert!(player2.movement.is_on_ground);
        assert_eq!(player2.spawn.spawn_pos, BlockPos::new(10, 65, -20));
    }

    #[test]
    fn test_creative_player_abilities() {
        let player = ServerPlayer::new(
            1,
            GameProfile::new(Uuid::new_v4(), "Creative".into()),
            ResourceLocation::minecraft("overworld"),
            GameMode::Creative,
        );
        assert!(player.abilities.is_invulnerable);
        assert!(player.abilities.can_fly);
        assert!(player.abilities.is_instabuild);
    }

    #[test]
    fn test_save_inventory_to_nbt() {
        let mut player = make_test_player(1, "Test");
        player
            .inventory
            .set(0, ItemStack::new("minecraft:diamond_sword", 1));
        player
            .inventory
            .set(9, ItemStack::new("minecraft:stone", 64));

        let nbt = player.save_to_nbt();
        let inv = nbt.get_list("Inventory").expect("Inventory tag missing");
        assert_eq!(inv.len(), 2);

        let items: Vec<_> = inv.iter().collect();

        // Check slot 0
        let NbtTag::Compound(item) = &items[0] else {
            unreachable!("Expected compound tag");
        };
        assert_eq!(item.get_byte("Slot"), Some(0));
        assert_eq!(item.get_string("id"), Some("minecraft:diamond_sword"));
        assert_eq!(item.get_int("count"), Some(1));

        // Check slot 9
        let NbtTag::Compound(item) = &items[1] else {
            unreachable!("Expected compound tag");
        };
        assert_eq!(item.get_byte("Slot"), Some(9));
        assert_eq!(item.get_string("id"), Some("minecraft:stone"));
        assert_eq!(item.get_int("count"), Some(64));
    }

    #[test]
    fn test_load_inventory_from_nbt() {
        let mut nbt = NbtCompound::new();
        let mut inv_list = NbtList::new(TAG_COMPOUND);

        let mut item0 = NbtCompound::new();
        item0.put_byte("Slot", 0);
        item0.put_string("id", "minecraft:iron_pickaxe");
        item0.put_byte("count", 1);
        let _ = inv_list.push(NbtTag::Compound(item0));

        let mut item9 = NbtCompound::new();
        item9.put_byte("Slot", 9);
        item9.put_string("id", "minecraft:dirt");
        item9.put_byte("count", 32);
        let _ = inv_list.push(NbtTag::Compound(item9));

        nbt.put("Inventory", NbtTag::List(inv_list));
        nbt.put_int("SelectedItemSlot", 3);

        let mut player = make_test_player(1, "Test");
        player.load_from_nbt(&nbt);

        assert_eq!(player.inventory.selected_slot, 3);
        let slot0 = player.inventory.get(0);
        assert_eq!(slot0.item.0, "minecraft:iron_pickaxe");
        assert_eq!(slot0.count, 1);
        let slot9 = player.inventory.get(9);
        assert_eq!(slot9.item.0, "minecraft:dirt");
        assert_eq!(slot9.count, 32);
        assert!(player.inventory.get(1).is_empty());
    }

    #[test]
    fn test_inventory_nbt_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player
            .inventory
            .set(0, ItemStack::new("minecraft:diamond_sword", 1));
        player
            .inventory
            .set(9, ItemStack::new("minecraft:stone", 64));
        player
            .inventory
            .set(40, ItemStack::new("minecraft:shield", 1));
        player.inventory.selected_slot = 5;

        let nbt = player.save_to_nbt();

        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert_eq!(player2.inventory.selected_slot, 5);
        assert_eq!(player2.inventory.get(0).item.0, "minecraft:diamond_sword");
        assert_eq!(player2.inventory.get(0).count, 1);
        assert_eq!(player2.inventory.get(9).item.0, "minecraft:stone");
        assert_eq!(player2.inventory.get(9).count, 64);
        assert_eq!(player2.inventory.get(40).item.0, "minecraft:shield");
        assert_eq!(player2.inventory.get(40).count, 1);
        assert!(player2.inventory.get(1).is_empty());
    }

    #[test]
    fn test_empty_inventory_nbt() {
        let player = make_test_player(1, "Test");
        let nbt = player.save_to_nbt();

        let inv = nbt.get_list("Inventory").expect("Inventory tag missing");
        assert_eq!(inv.len(), 0);

        assert_eq!(nbt.get_int("SelectedItemSlot"), Some(0));
    }

    #[test]
    fn test_load_invalid_slot_ignored() {
        let mut nbt = NbtCompound::new();
        let mut inv_list = NbtList::new(TAG_COMPOUND);

        // Slot 255 — out of bounds, should be ignored
        let mut bad = NbtCompound::new();
        bad.put_byte("Slot", 127); // max positive i8 = 127 > 40
        bad.put_string("id", "minecraft:stone");
        bad.put_byte("count", 1);
        let _ = inv_list.push(NbtTag::Compound(bad));

        nbt.put("Inventory", NbtTag::List(inv_list));

        let mut player = make_test_player(1, "Test");
        player.load_from_nbt(&nbt);

        // All slots should still be empty
        for i in 0..PlayerInventory::TOTAL_SLOTS {
            assert!(player.inventory.get(i).is_empty());
        }
    }

    #[test]
    fn test_xp_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player.experience.xp_level = 30;
        player.experience.xp_progress = 0.75;
        player.experience.xp_total = 1395;
        player.experience.xp_seed = 42;

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert_eq!(player2.experience.xp_level, 30);
        assert!((player2.experience.xp_progress - 0.75).abs() < f32::EPSILON);
        assert_eq!(player2.experience.xp_total, 1395);
        assert_eq!(player2.experience.xp_seed, 42);
    }

    #[test]
    fn test_score_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player.combat.score = 500;

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert_eq!(player2.combat.score, 500);
    }

    #[test]
    fn test_absorption_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player.combat.absorption_amount = 8.0;

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert!((player2.combat.absorption_amount - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fall_flying_roundtrip() {
        let mut player = make_test_player(1, "Test");
        player.movement.is_fall_flying = true;

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert!(player2.movement.is_fall_flying);
    }

    #[test]
    fn test_abilities_roundtrip_preserves_custom_values() {
        let mut player = make_test_player(1, "Test");
        player.abilities.is_flying = true;
        player.abilities.can_fly = true;
        player.abilities.fly_speed = 0.10;
        player.abilities.walk_speed = 0.15;

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        assert!(player2.abilities.is_flying);
        assert!(player2.abilities.can_fly);
        assert!((player2.abilities.fly_speed - 0.10).abs() < f32::EPSILON);
        assert!((player2.abilities.walk_speed - 0.15).abs() < f32::EPSILON);
    }

    #[test]
    fn test_last_death_location_roundtrip() {
        let mut player = make_test_player(1, "Test");
        let dim = ResourceLocation::minecraft("overworld");
        let packed_pos: i64 =
            ((100_i64 & 0x3FF_FFFF) << 38) | (((-200_i64) & 0x3FF_FFFF) << 12) | (64_i64 & 0xFFF);
        player.combat.last_death_location = Some((dim.clone(), packed_pos));

        let nbt = player.save_to_nbt();
        let mut player2 = make_test_player(2, "Test2");
        player2.load_from_nbt(&nbt);

        let (dim2, pos2) = player2.combat.last_death_location.unwrap();
        assert_eq!(dim2, dim);
        assert_eq!(pos2, packed_pos);
    }

    #[test]
    fn test_raw_nbt_roundtrip_active_effects() {
        let mut player = make_test_player(1, "Test");

        // Simulate raw active_effects tag from a vanilla world
        let mut effect = NbtCompound::new();
        effect.put_byte("id", 1);
        effect.put_byte("amplifier", 0);
        effect.put_int("duration", 600);

        let mut effects_list = NbtList::new(TAG_COMPOUND);
        let _ = effects_list.push(NbtTag::Compound(effect));

        let mut nbt = NbtCompound::new();
        nbt.put("active_effects", NbtTag::List(effects_list));

        player.load_from_nbt(&nbt);
        assert!(player.raw_nbt.active_effects.is_some());

        let saved = player.save_to_nbt();
        assert!(saved.get("active_effects").is_some());
    }

    #[test]
    fn test_raw_nbt_roundtrip_ender_items() {
        let mut player = make_test_player(1, "Test");

        let mut item = NbtCompound::new();
        item.put_string("id", "minecraft:diamond");
        item.put_byte("count", 64);
        item.put_byte("Slot", 0);

        let mut ender_list = NbtList::new(TAG_COMPOUND);
        let _ = ender_list.push(NbtTag::Compound(item));

        let mut nbt = NbtCompound::new();
        nbt.put("EnderItems", NbtTag::List(ender_list));

        player.load_from_nbt(&nbt);
        assert!(player.raw_nbt.ender_items.is_some());

        let saved = player.save_to_nbt();
        let ender = saved.get("EnderItems").unwrap();
        if let NbtTag::List(list) = ender {
            assert_eq!(list.len(), 1);
        } else {
            panic!("EnderItems should be a list");
        }
    }

    #[test]
    fn test_raw_nbt_roundtrip_attributes() {
        let mut player = make_test_player(1, "Test");

        let mut attr = NbtCompound::new();
        attr.put_string("Name", "minecraft:generic.max_health");
        attr.put_double("Base", 20.0);

        let mut attr_list = NbtList::new(TAG_COMPOUND);
        let _ = attr_list.push(NbtTag::Compound(attr));

        let mut nbt = NbtCompound::new();
        nbt.put("attributes", NbtTag::List(attr_list));

        player.load_from_nbt(&nbt);
        assert!(player.raw_nbt.attributes.is_some());

        let saved = player.save_to_nbt();
        assert!(saved.get("attributes").is_some());
    }

    #[test]
    fn test_xp_progress_clamped() {
        let mut player = make_test_player(1, "Test");
        let mut nbt = NbtCompound::new();
        nbt.put_float("XpP", 2.5); // out of range

        player.load_from_nbt(&nbt);
        assert!((player.experience.xp_progress - 1.0).abs() < f32::EPSILON);
    }
}
