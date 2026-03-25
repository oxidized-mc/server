//! Clientbound login packet — first packet in PLAY state.
//!
//! Contains world metadata, player entity ID, and spawn information.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundLoginPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::{types, varint};
use crate::types::resource_location::ResourceLocation;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Common spawn information shared between login and respawn packets.
///
/// Mirrors `net.minecraft.network.protocol.game.CommonPlayerSpawnInfo`.
///
/// The `dimension_type_id` field uses vanilla's `Holder<DimensionType>` encoding
/// (`ByteBufCodecs.holderRegistry`), which writes `VarInt(registry_id + 1)` on the
/// wire. Value 0 is reserved for inline definitions. Callers should pass the
/// **raw registry ID** (0 = overworld); encoding adds the +1 automatically.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonPlayerSpawnInfo {
    /// Dimension type registry ID (0-based). 0 = overworld, 1 = overworld_caves, etc.
    pub dimension_type_id: i32,
    /// Dimension the player is in.
    pub dimension: ResourceLocation,
    /// Hashed seed for biome noise.
    pub seed: i64,
    /// Current game mode (0–3).
    pub game_mode: u8,
    /// Previous game mode (-1 = none, 0–3).
    pub previous_game_mode: i8,
    /// Whether this is a debug world.
    pub is_debug: bool,
    /// Whether this is a superflat world.
    pub is_flat: bool,
    /// Location of last death: Optional (dimension ResourceLocation, packed BlockPos i64).
    pub last_death_location: Option<(ResourceLocation, i64)>,
    /// Portal cooldown in ticks.
    pub portal_cooldown: i32,
    /// Sea level height.
    pub sea_level: i32,
}

impl CommonPlayerSpawnInfo {
    /// Decodes from a buffer.
    ///
    /// # Errors
    ///
    /// Returns [`PacketDecodeError`] if the buffer is truncated or contains
    /// invalid dimension/gamemode data.
    pub fn decode(data: &mut Bytes) -> Result<Self, PacketDecodeError> {
        // Holder<DimensionType> wire encoding: VarInt(registry_id + 1).
        let raw_holder_id = varint::read_varint_buf(data)?;
        if raw_holder_id <= 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "inline dimension type holders not supported (got {raw_holder_id})"
            )));
        }
        let dimension_type_id = raw_holder_id - 1;
        let dimension = ResourceLocation::read(data)?;
        let seed = types::read_i64(data)?;
        types::ensure_remaining(data, 1, "CommonPlayerSpawnInfo game_mode")?;
        let game_mode = data.get_u8();
        types::ensure_remaining(data, 1, "CommonPlayerSpawnInfo previous_game_mode")?;
        let previous_game_mode = data.get_i8();
        let is_debug = types::read_bool(data)?;
        let is_flat = types::read_bool(data)?;

        let has_death_loc = types::read_bool(data)?;
        let last_death_location = if has_death_loc {
            let dim = ResourceLocation::read(data)?;
            let pos = types::read_i64(data)?;
            Some((dim, pos))
        } else {
            None
        };

        let portal_cooldown = varint::read_varint_buf(data)?;
        let sea_level = varint::read_varint_buf(data)?;

        Ok(Self {
            dimension_type_id,
            dimension,
            seed,
            game_mode,
            previous_game_mode,
            is_debug,
            is_flat,
            last_death_location,
            portal_cooldown,
            sea_level,
        })
    }

    /// Encodes into a buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        // Holder<DimensionType> wire encoding: VarInt(registry_id + 1).
        // 0 is reserved for inline holders; 1+ are registry references.
        varint::write_varint_buf(self.dimension_type_id + 1, buf);
        self.dimension.write(buf);
        types::write_i64(buf, self.seed);
        buf.put_u8(self.game_mode);
        buf.put_i8(self.previous_game_mode);
        types::write_bool(buf, self.is_debug);
        types::write_bool(buf, self.is_flat);

        match &self.last_death_location {
            Some((dim, pos)) => {
                types::write_bool(buf, true);
                dim.write(buf);
                types::write_i64(buf, *pos);
            },
            None => {
                types::write_bool(buf, false);
            },
        }

        varint::write_varint_buf(self.portal_cooldown, buf);
        varint::write_varint_buf(self.sea_level, buf);
    }
}

/// The first PLAY-state packet, establishing the game world.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundLoginPacket {
    /// The player's entity ID.
    pub player_id: i32,
    /// Whether this is a is_hardcore world.
    pub is_hardcore: bool,
    /// All dimension identifiers.
    pub dimensions: Vec<ResourceLocation>,
    /// Maximum number of players (display only).
    pub max_players: i32,
    /// View distance in chunks.
    pub chunk_radius: i32,
    /// Simulation distance in chunks.
    pub simulation_distance: i32,
    /// Whether reduced debug info is enabled.
    pub has_reduced_debug_info: bool,
    /// Whether to show the death screen.
    pub is_showing_death_screen: bool,
    /// Whether limited crafting is enabled.
    pub is_limited_crafting: bool,
    /// Spawn information for the initial dimension.
    pub common_spawn_info: CommonPlayerSpawnInfo,
    /// Whether the server enforces secure chat.
    pub is_secure_chat_enforced: bool,
}

impl Packet for ClientboundLoginPacket {
    const PACKET_ID: i32 = 0x31;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let player_id = types::read_i32(&mut data)?;
        let is_hardcore = types::read_bool(&mut data)?;

        let dim_count = varint::read_varint_buf(&mut data)? as usize;
        let mut dimensions = Vec::with_capacity(dim_count);
        for _ in 0..dim_count {
            dimensions.push(ResourceLocation::read(&mut data)?);
        }

        let max_players = varint::read_varint_buf(&mut data)?;
        let chunk_radius = varint::read_varint_buf(&mut data)?;
        let simulation_distance = varint::read_varint_buf(&mut data)?;
        let has_reduced_debug_info = types::read_bool(&mut data)?;
        let is_showing_death_screen = types::read_bool(&mut data)?;
        let is_limited_crafting = types::read_bool(&mut data)?;
        let common_spawn_info = CommonPlayerSpawnInfo::decode(&mut data)?;
        let is_secure_chat_enforced = types::read_bool(&mut data)?;

        Ok(Self {
            player_id,
            is_hardcore,
            dimensions,
            max_players,
            chunk_radius,
            simulation_distance,
            has_reduced_debug_info,
            is_showing_death_screen,
            is_limited_crafting,
            common_spawn_info,
            is_secure_chat_enforced,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(128);
        types::write_i32(&mut buf, self.player_id);
        types::write_bool(&mut buf, self.is_hardcore);

        types::write_list(&mut buf, &self.dimensions, |b, d| d.write(b));

        varint::write_varint_buf(self.max_players, &mut buf);
        varint::write_varint_buf(self.chunk_radius, &mut buf);
        varint::write_varint_buf(self.simulation_distance, &mut buf);
        types::write_bool(&mut buf, self.has_reduced_debug_info);
        types::write_bool(&mut buf, self.is_showing_death_screen);
        types::write_bool(&mut buf, self.is_limited_crafting);
        self.common_spawn_info.encode(&mut buf);
        types::write_bool(&mut buf, self.is_secure_chat_enforced);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample_login_packet() -> ClientboundLoginPacket {
        ClientboundLoginPacket {
            player_id: 42,
            is_hardcore: false,
            dimensions: vec![
                ResourceLocation::minecraft("overworld"),
                ResourceLocation::minecraft("the_nether"),
                ResourceLocation::minecraft("the_end"),
            ],
            max_players: 20,
            chunk_radius: 10,
            simulation_distance: 10,
            has_reduced_debug_info: false,
            is_showing_death_screen: true,
            is_limited_crafting: false,
            common_spawn_info: CommonPlayerSpawnInfo {
                dimension_type_id: 0,
                dimension: ResourceLocation::minecraft("overworld"),
                seed: 0,
                game_mode: 0,
                previous_game_mode: -1,
                is_debug: false,
                is_flat: false,
                last_death_location: None,
                portal_cooldown: 0,
                sea_level: 63,
            },
            is_secure_chat_enforced: false,
        }
    }

    #[test]
    fn test_login_packet_roundtrip() {
        let pkt = sample_login_packet();
        let encoded = pkt.encode();
        let decoded = ClientboundLoginPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.player_id, 42);
        assert!(!decoded.is_hardcore);
        assert_eq!(decoded.dimensions.len(), 3);
        assert_eq!(decoded.max_players, 20);
        assert_eq!(decoded.chunk_radius, 10);
        assert_eq!(decoded.simulation_distance, 10);
        assert!(!decoded.has_reduced_debug_info);
        assert!(decoded.is_showing_death_screen);
        assert!(!decoded.is_limited_crafting);
        assert_eq!(decoded.common_spawn_info.game_mode, 0);
        assert_eq!(decoded.common_spawn_info.previous_game_mode, -1);
        assert_eq!(decoded.common_spawn_info.sea_level, 63);
        assert!(!decoded.is_secure_chat_enforced);
    }

    #[test]
    fn test_login_packet_with_death_location() {
        let mut pkt = sample_login_packet();
        let death_pos = crate::types::block_pos::BlockPos::new(100, 64, -200);
        pkt.common_spawn_info.last_death_location = Some((
            ResourceLocation::minecraft("overworld"),
            death_pos.as_long(),
        ));

        let encoded = pkt.encode();
        let decoded = ClientboundLoginPacket::decode(encoded.freeze()).unwrap();

        let (dim, pos_packed) = decoded.common_spawn_info.last_death_location.unwrap();
        assert_eq!(dim, ResourceLocation::minecraft("overworld"));
        let death = crate::types::block_pos::BlockPos::from_long(pos_packed);
        assert_eq!(death.x, 100);
        assert_eq!(death.y, 64);
        assert_eq!(death.z, -200);
    }

    #[test]
    fn test_login_packet_hardcore() {
        let mut pkt = sample_login_packet();
        pkt.is_hardcore = true;
        let encoded = pkt.encode();
        let decoded = ClientboundLoginPacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.is_hardcore);
    }
}
