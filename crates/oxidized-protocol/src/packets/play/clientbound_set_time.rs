//! ClientboundSetTimePacket (0x71) — world time update.
//!
//! Sent every 20 ticks to synchronise the client's game time and clock
//! states. The `game_time` field is the total world age (monotonically
//! increasing). The `clock_updates` map carries per-clock state changes
//! (e.g., the overworld day/night clock).
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundSetTimePacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::Packet;
use crate::codec::packet::PacketDecodeError;
use crate::codec::{types, varint};

/// Network state for a single world clock.
///
/// Mirrors `net.minecraft.world.clock.ClockNetworkState`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockNetworkState {
    /// Total ticks elapsed on this clock.
    pub total_ticks: i64,
    /// Fractional tick for smooth interpolation.
    pub partial_tick: f32,
    /// Tick rate multiplier (0.0 = paused, 1.0 = normal speed).
    pub rate: f32,
}

/// A single clock update entry in the time packet.
#[derive(Debug, Clone, PartialEq)]
pub struct ClockUpdate {
    /// Registry network ID of the `WorldClock` holder (VarInt-encoded as id + 1).
    pub clock_id: i32,
    /// The clock's current state.
    pub state: ClockNetworkState,
}

/// 0x71 — World time synchronisation packet.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundSetTimePacket {
    /// Absolute world age in ticks (never resets).
    pub game_time: i64,
    /// Per-clock state updates. Empty for periodic sync; populated on join
    /// or when a clock changes (e.g., `/time set`).
    pub clock_updates: Vec<ClockUpdate>,
}

impl ClientboundSetTimePacket {
    /// Overworld clock network ID (first entry in the `world_clock` registry).
    pub const OVERWORLD_CLOCK_ID: i32 = 0;
}

/// Writes a VarLong to a buffer.
fn write_varlong(buf: &mut BytesMut, mut value: i64) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.put_u8(byte);
            break;
        }
        buf.put_u8(byte | 0x80);
    }
}

/// Reads a VarLong from a buffer.
fn read_varlong(data: &mut Bytes) -> Result<i64, PacketDecodeError> {
    let mut result: i64 = 0;
    let mut shift = 0u32;
    loop {
        if !data.has_remaining() {
            return Err(PacketDecodeError::InvalidData(
                "VarLong truncated".into(),
            ));
        }
        let byte = data.get_u8();
        result |= i64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 70 {
            return Err(PacketDecodeError::InvalidData(
                "VarLong too long".to_string(),
            ));
        }
    }
}

impl Packet for ClientboundSetTimePacket {
    const PACKET_ID: i32 = 0x71;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let game_time = types::read_i64(&mut data)?;

        let map_size = varint::read_varint_buf(&mut data)?;
        let mut clock_updates = Vec::with_capacity(map_size as usize);
        for _ in 0..map_size {
            let holder_id = varint::read_varint_buf(&mut data)?;
            let clock_id = holder_id - 1;
            let total_ticks = read_varlong(&mut data)?;
            types::ensure_remaining(&data, 8, "ClockNetworkState floats")?;
            let partial_tick = data.get_f32();
            let rate = data.get_f32();
            clock_updates.push(ClockUpdate {
                clock_id,
                state: ClockNetworkState {
                    total_ticks,
                    partial_tick,
                    rate,
                },
            });
        }

        Ok(Self {
            game_time,
            clock_updates,
        })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(8 + 5 + self.clock_updates.len() * 20);
        buf.put_i64(self.game_time);
        varint::write_varint_buf(self.clock_updates.len() as i32, &mut buf);
        for update in &self.clock_updates {
            // Holder encoding: registry ID + 1
            varint::write_varint_buf(update.clock_id + 1, &mut buf);
            write_varlong(&mut buf, update.state.total_ticks);
            buf.put_f32(update.state.partial_tick);
            buf.put_f32(update.state.rate);
        }
        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_empty_clocks() {
        let pkt = ClientboundSetTimePacket {
            game_time: 72000,
            clock_updates: vec![],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetTimePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_with_overworld_clock() {
        let pkt = ClientboundSetTimePacket {
            game_time: 6000,
            clock_updates: vec![ClockUpdate {
                clock_id: ClientboundSetTimePacket::OVERWORLD_CLOCK_ID,
                state: ClockNetworkState {
                    total_ticks: 6000,
                    partial_tick: 0.0,
                    rate: 1.0,
                },
            }],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetTimePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_multiple_clocks() {
        let pkt = ClientboundSetTimePacket {
            game_time: 100_000,
            clock_updates: vec![
                ClockUpdate {
                    clock_id: 0,
                    state: ClockNetworkState {
                        total_ticks: 4000,
                        partial_tick: 0.5,
                        rate: 1.0,
                    },
                },
                ClockUpdate {
                    clock_id: 1,
                    state: ClockNetworkState {
                        total_ticks: 18000,
                        partial_tick: 0.0,
                        rate: 0.0,
                    },
                },
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundSetTimePacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_packet_id() {
        assert_packet_id!(ClientboundSetTimePacket, 0x71);
    }
}
