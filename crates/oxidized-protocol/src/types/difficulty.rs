//! [`Difficulty`] — the difficulty level of the game.
//!
//! Maps to the vanilla `Difficulty` enum used in server-properties,
//! login/join-game packets, and difficulty-change packets.

/// The difficulty level of the game.
///
/// # Wire format
///
/// Encoded as a VarInt (0–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Difficulty {
    /// Peaceful — no hostile mobs, health regenerates.
    Peaceful = 0,
    /// Easy — hostile mobs deal less damage.
    Easy = 1,
    /// Normal — default difficulty.
    Normal = 2,
    /// Hard — hostile mobs deal more damage, hunger can kill.
    Hard = 3,
}

impl_protocol_enum! {
    Difficulty {
        Peaceful = 0 => "peaceful",
        Easy     = 1 => "easy",
        Normal   = 2 => "normal",
        Hard     = 3 => "hard",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::BytesMut;

    use super::*;

    // ── by_id ───────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_by_id_valid() {
        assert_eq!(Difficulty::by_id(0), Some(Difficulty::Peaceful));
        assert_eq!(Difficulty::by_id(1), Some(Difficulty::Easy));
        assert_eq!(Difficulty::by_id(2), Some(Difficulty::Normal));
        assert_eq!(Difficulty::by_id(3), Some(Difficulty::Hard));
    }

    #[test]
    fn test_difficulty_by_id_invalid() {
        assert_eq!(Difficulty::by_id(-1), None);
        assert_eq!(Difficulty::by_id(4), None);
        assert_eq!(Difficulty::by_id(100), None);
    }

    // ── by_name ─────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_by_name_valid() {
        assert_eq!(Difficulty::by_name("peaceful"), Some(Difficulty::Peaceful));
        assert_eq!(Difficulty::by_name("easy"), Some(Difficulty::Easy));
        assert_eq!(Difficulty::by_name("normal"), Some(Difficulty::Normal));
        assert_eq!(Difficulty::by_name("hard"), Some(Difficulty::Hard));
    }

    #[test]
    fn test_difficulty_by_name_invalid() {
        assert_eq!(Difficulty::by_name("Peaceful"), None);
        assert_eq!(Difficulty::by_name("unknown"), None);
        assert_eq!(Difficulty::by_name(""), None);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_difficulty_display() {
        assert_eq!(format!("{}", Difficulty::Peaceful), "peaceful");
        assert_eq!(format!("{}", Difficulty::Easy), "easy");
        assert_eq!(format!("{}", Difficulty::Normal), "normal");
        assert_eq!(format!("{}", Difficulty::Hard), "hard");
    }

    // ── Roundtrip ───────────────────────────────────────────────────

    #[test]
    fn test_difficulty_id_roundtrip() {
        for id in 0..=3 {
            let d = Difficulty::by_id(id).unwrap();
            assert_eq!(d.id(), id);
        }
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_difficulty_wire_roundtrip() {
        for id in 0..=3 {
            let d = Difficulty::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            d.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = Difficulty::read(&mut data).unwrap();
            assert_eq!(decoded, d);
        }
    }
}
