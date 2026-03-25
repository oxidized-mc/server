//! Clientbound game event packet.
//!
//! Notifies the client of various game events: game mode changes,
//! weather transitions, respawn screen options, etc.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundGameEventPacket`.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::types;

/// A game event type ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GameEventType {
    /// No respawn block available.
    NoRespawnBlockAvailable = 0,
    /// Rain starts.
    StartRaining = 1,
    /// Rain stops.
    StopRaining = 2,
    /// Game mode changed (param = game mode ID as float).
    ChangeGameMode = 3,
    /// Player won the game.
    WinGame = 4,
    /// Demo event.
    DemoEvent = 5,
    /// Arrow hit player sound.
    PlayArrowHitSound = 6,
    /// Rain level change (param = intensity 0.0–1.0).
    RainLevelChange = 7,
    /// Thunder level change (param = intensity 0.0–1.0).
    ThunderLevelChange = 8,
    /// Puffer fish sting.
    PufferFishSting = 9,
    /// Guardian elder effect.
    GuardianElderEffect = 10,
    /// Immediate respawn (param = 0 or 1).
    ImmediateRespawn = 11,
    /// Limited crafting (param = 0 or 1).
    LimitedCrafting = 12,
    /// Level chunks load start.
    LevelChunksLoadStart = 13,
}

impl GameEventType {
    /// Converts from a raw byte ID.
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::NoRespawnBlockAvailable),
            1 => Some(Self::StartRaining),
            2 => Some(Self::StopRaining),
            3 => Some(Self::ChangeGameMode),
            4 => Some(Self::WinGame),
            5 => Some(Self::DemoEvent),
            6 => Some(Self::PlayArrowHitSound),
            7 => Some(Self::RainLevelChange),
            8 => Some(Self::ThunderLevelChange),
            9 => Some(Self::PufferFishSting),
            10 => Some(Self::GuardianElderEffect),
            11 => Some(Self::ImmediateRespawn),
            12 => Some(Self::LimitedCrafting),
            13 => Some(Self::LevelChunksLoadStart),
            _ => None,
        }
    }
}

/// Clientbound packet for game-level events.
///
/// Wire format: `event_type: u8 | param: f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundGameEventPacket {
    /// The event type.
    pub event: GameEventType,
    /// Event-specific parameter.
    pub param: f32,
}

impl Packet for ClientboundGameEventPacket {
    const PACKET_ID: i32 = 0x26;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        types::ensure_remaining(&data, 5, "GameEventPacket")?;
        let type_id = types::read_u8(&mut data)?;
        let param = types::read_f32(&mut data)?;
        let event = GameEventType::from_id(type_id).ok_or_else(|| {
            PacketDecodeError::InvalidData(format!("unknown game event type: {type_id}"))
        })?;
        Ok(Self { event, param })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u8(self.event as u8);
        buf.put_f32(self.param);
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_change_game_mode() {
        let pkt = ClientboundGameEventPacket {
            event: GameEventType::ChangeGameMode,
            param: 1.0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundGameEventPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded.event, GameEventType::ChangeGameMode);
        assert!((decoded.param - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_unknown_event_type() {
        let mut buf = BytesMut::with_capacity(5);
        buf.put_u8(255);
        buf.put_f32(0.0);
        let result = ClientboundGameEventPacket::decode(buf.freeze());
        assert!(result.is_err());
    }
}
