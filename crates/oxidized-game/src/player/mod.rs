//! Player management: state, abilities, inventory, and the player list.
//!
//! This module owns the per-player runtime state (`ServerPlayer`) and the
//! server-wide player roster (`PlayerList`). Game mode and ability
//! configuration are also defined here.

pub mod abilities;
pub mod game_mode;
pub mod inventory;
pub mod login;
pub mod movement;
pub mod player_list;
pub mod server_player;

pub use abilities::PlayerAbilities;
pub use game_mode::GameMode;
pub use inventory::PlayerInventory;
pub use login::{
    EncodedPacket, build_container_set_content_packet, build_login_sequence,
    build_spawn_position_packet, handle_accept_teleportation,
};
pub use player_list::PlayerList;
pub use server_player::{
    CombatStats, ConnectionInfo, MiningState, PlayerExperience, PlayerMovement, RawPlayerNbt,
    ServerPlayer, SpawnInfo, TeleportTracker,
};
