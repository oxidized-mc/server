//! [`ParticleStatus`] — controls the particle rendering level on the client.
//!
//! Maps to the vanilla `ParticleStatus` enum.
//! Used in [`ServerboundClientInformationPacket`] during configuration.

use std::fmt;

use bytes::{Bytes, BytesMut};

use crate::codec::types::TypeError;
use crate::codec::varint;

/// Controls the particle rendering level on the client.
///
/// # Wire format
///
/// Encoded as a VarInt (0–2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ParticleStatus {
    /// Show all particles.
    All = 0,
    /// Show decreased particles.
    Decreased = 1,
    /// Show minimal particles.
    Minimal = 2,
}

impl ParticleStatus {
    /// Returns the numeric ID of this particle status.
    pub const fn id(self) -> i32 {
        self as i32
    }

    /// Returns the lowercase name of this particle status.
    pub const fn name(self) -> &'static str {
        match self {
            ParticleStatus::All => "all",
            ParticleStatus::Decreased => "decreased",
            ParticleStatus::Minimal => "minimal",
        }
    }

    /// Looks up a particle status by numeric ID.
    ///
    /// Returns `None` if `id` is not in 0–2.
    pub const fn by_id(id: i32) -> Option<ParticleStatus> {
        match id {
            0 => Some(ParticleStatus::All),
            1 => Some(ParticleStatus::Decreased),
            2 => Some(ParticleStatus::Minimal),
            _ => None,
        }
    }

    /// Reads a [`ParticleStatus`] from a wire buffer as a VarInt.
    ///
    /// # Errors
    ///
    /// Returns [`TypeError`] if the buffer is truncated or the value is
    /// out of range.
    pub fn read(buf: &mut Bytes) -> Result<Self, TypeError> {
        let id = varint::read_varint_buf(buf)?;
        ParticleStatus::by_id(id).ok_or(TypeError::UnexpectedEof { need: 1, have: 0 })
    }

    /// Writes this [`ParticleStatus`] to a wire buffer as a VarInt.
    pub fn write(&self, buf: &mut BytesMut) {
        varint::write_varint_buf(self.id(), buf);
    }
}

impl fmt::Display for ParticleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── by_id ───────────────────────────────────────────────────────

    #[test]
    fn test_particle_status_by_id_valid() {
        assert_eq!(ParticleStatus::by_id(0), Some(ParticleStatus::All));
        assert_eq!(ParticleStatus::by_id(1), Some(ParticleStatus::Decreased));
        assert_eq!(ParticleStatus::by_id(2), Some(ParticleStatus::Minimal));
    }

    #[test]
    fn test_particle_status_by_id_invalid() {
        assert_eq!(ParticleStatus::by_id(-1), None);
        assert_eq!(ParticleStatus::by_id(3), None);
        assert_eq!(ParticleStatus::by_id(100), None);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_particle_status_display() {
        assert_eq!(format!("{}", ParticleStatus::All), "all");
        assert_eq!(format!("{}", ParticleStatus::Decreased), "decreased");
        assert_eq!(format!("{}", ParticleStatus::Minimal), "minimal");
    }

    // ── Roundtrip id ↔ enum ─────────────────────────────────────────

    #[test]
    fn test_particle_status_id_roundtrip() {
        for id in 0..=2 {
            let ps = ParticleStatus::by_id(id).unwrap();
            assert_eq!(ps.id(), id);
        }
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_particle_status_wire_roundtrip() {
        for id in 0..=2 {
            let ps = ParticleStatus::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            ps.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = ParticleStatus::read(&mut data).unwrap();
            assert_eq!(decoded, ps);
        }
    }

    #[test]
    fn test_particle_status_read_empty_buffer() {
        let mut data = Bytes::new();
        assert!(ParticleStatus::read(&mut data).is_err());
    }
}
