//! [`ParticleStatus`] — controls the particle rendering level on the client.
//!
//! Maps to the vanilla `ParticleStatus` enum.
//! Used in [`ServerboundClientInformationPacket`] during configuration.

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

impl_protocol_enum! {
    ParticleStatus {
        All       = 0 => "all",
        Decreased = 1 => "decreased",
        Minimal   = 2 => "minimal",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::{Bytes, BytesMut};

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
