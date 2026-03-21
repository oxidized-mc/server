//! Clientbound login packet — first packet in PLAY state.
//!
//! Contains world metadata, player entity ID, and spawn information.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundLoginPacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::types;
use crate::codec::varint;
use crate::types::resource_location::ResourceLocation;

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;

/// Common spawn information shared between login and respawn packets.
///
/// Mirrors `net.minecraft.network.protocol.game.CommonPlayerSpawnInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct CommonPlayerSpawnInfo {
    /// Dimension type registry ID (VarInt). 0 = overworld, 1 = the_nether, 2 = the_end.
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
    pub fn decode(data: &mut Bytes) -> Result<Self, PacketDecodeError> {
        let dimension_type_id = varint::read_varint_buf(data)?;
        let dimension = ResourceLocation::read(data)?;
        let seed = types::read_i64(data)?;
        if data.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData(
                "unexpected end of packet data".into(),
            ));
        }
        let game_mode = data.get_u8();
        if data.remaining() < 1 {
            return Err(PacketDecodeError::InvalidData(
                "unexpected end of packet data".into(),
            ));
        }
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
        varint::write_varint_buf(self.dimension_type_id, buf);
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
    /// Whether this is a hardcore world.
    pub hardcore: bool,
    /// All dimension identifiers.
    pub dimensions: Vec<ResourceLocation>,
    /// Maximum number of players (display only).
    pub max_players: i32,
    /// View distance in chunks.
    pub chunk_radius: i32,
    /// Simulation distance in chunks.
    pub simulation_distance: i32,
    /// Whether reduced debug info is enabled.
    pub reduced_debug_info: bool,
    /// Whether to show the death screen.
    pub show_death_screen: bool,
    /// Whether limited crafting is enabled.
    pub do_limited_crafting: bool,
    /// Spawn information for the initial dimension.
    pub common_spawn_info: CommonPlayerSpawnInfo,
    /// Whether the server enforces secure chat.
    pub enforces_secure_chat: bool,
}

impl Packet for ClientboundLoginPacket {
    const PACKET_ID: i32 = 0x31;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let player_id = types::read_i32(&mut data)?;
        let hardcore = types::read_bool(&mut data)?;

        let dim_count = varint::read_varint_buf(&mut data)? as usize;
        let mut dimensions = Vec::with_capacity(dim_count);
        for _ in 0..dim_count {
            dimensions.push(ResourceLocation::read(&mut data)?);
        }

        let max_players = varint::read_varint_buf(&mut data)?;
        let chunk_radius = varint::read_varint_buf(&mut data)?;
        let simulation_distance = varint::read_varint_buf(&mut data)?;
        let reduced_debug_info = types::read_bool(&mut data)?;
        let show_death_screen = types::read_bool(&mut data)?;
        let do_limited_crafting = types::read_bool(&mut data)?;
        let common_spawn_info = CommonPlayerSpawnInfo::decode(&mut data)?;
        let enforces_secure_chat = types::read_bool(&mut data)?;

        Ok(Self {
            player_id,
            hardcore,
            dimensions,
            max_players,
            chunk_radius,
            simulation_distance,
            reduced_debug_info,
            show_death_screen,
            do_limited_crafting,
            common_spawn_info,
            enforces_secure_chat,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(128);
        types::write_i32(&mut buf, self.player_id);
        types::write_bool(&mut buf, self.hardcore);

        varint::write_varint_buf(self.dimensions.len() as i32, &mut buf);
        for dim in &self.dimensions {
            dim.write(&mut buf);
        }

        varint::write_varint_buf(self.max_players, &mut buf);
        varint::write_varint_buf(self.chunk_radius, &mut buf);
        varint::write_varint_buf(self.simulation_distance, &mut buf);
        types::write_bool(&mut buf, self.reduced_debug_info);
        types::write_bool(&mut buf, self.show_death_screen);
        types::write_bool(&mut buf, self.do_limited_crafting);
        self.common_spawn_info.encode(&mut buf);
        types::write_bool(&mut buf, self.enforces_secure_chat);
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
            hardcore: false,
            dimensions: vec![
                ResourceLocation::minecraft("overworld"),
                ResourceLocation::minecraft("the_nether"),
                ResourceLocation::minecraft("the_end"),
            ],
            max_players: 20,
            chunk_radius: 10,
            simulation_distance: 10,
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
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
            enforces_secure_chat: false,
        }
    }

    #[test]
    fn test_login_packet_roundtrip() {
        let pkt = sample_login_packet();
        let encoded = pkt.encode();
        let decoded = ClientboundLoginPacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.player_id, 42);
        assert!(!decoded.hardcore);
        assert_eq!(decoded.dimensions.len(), 3);
        assert_eq!(decoded.max_players, 20);
        assert_eq!(decoded.chunk_radius, 10);
        assert_eq!(decoded.simulation_distance, 10);
        assert!(!decoded.reduced_debug_info);
        assert!(decoded.show_death_screen);
        assert!(!decoded.do_limited_crafting);
        assert_eq!(decoded.common_spawn_info.game_mode, 0);
        assert_eq!(decoded.common_spawn_info.previous_game_mode, -1);
        assert_eq!(decoded.common_spawn_info.sea_level, 63);
        assert!(!decoded.enforces_secure_chat);
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
        pkt.hardcore = true;
        let encoded = pkt.encode();
        let decoded = ClientboundLoginPacket::decode(encoded.freeze()).unwrap();
        assert!(decoded.hardcore);
    }
}
