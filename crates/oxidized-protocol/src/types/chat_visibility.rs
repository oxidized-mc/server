//! [`ChatVisibility`] — controls which chat messages the client wants to see.
//!
//! Maps to the vanilla `ChatVisiblity` enum (note: vanilla misspells it).
//! Used in [`ServerboundClientInformationPacket`](crate::packets::configuration::ServerboundClientInformationPacket) during configuration.

/// Controls which chat messages the client wants to receive.
///
/// # Wire format
///
/// Encoded as a VarInt (0–2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ChatVisibility {
    /// Show all chat messages.
    Full = 0,
    /// Show only system messages (no player chat).
    System = 1,
    /// Hide all chat messages.
    Hidden = 2,
}

impl_protocol_enum! {
    ChatVisibility {
        Full   = 0 => "full",
        System = 1 => "system",
        Hidden = 2 => "hidden",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bytes::{Bytes, BytesMut};

    use super::*;

    // ── by_id ───────────────────────────────────────────────────────

    #[test]
    fn test_chat_visibility_by_id_valid() {
        assert_eq!(ChatVisibility::by_id(0), Some(ChatVisibility::Full));
        assert_eq!(ChatVisibility::by_id(1), Some(ChatVisibility::System));
        assert_eq!(ChatVisibility::by_id(2), Some(ChatVisibility::Hidden));
    }

    #[test]
    fn test_chat_visibility_by_id_invalid() {
        assert_eq!(ChatVisibility::by_id(-1), None);
        assert_eq!(ChatVisibility::by_id(3), None);
        assert_eq!(ChatVisibility::by_id(100), None);
    }

    // ── Display ─────────────────────────────────────────────────────

    #[test]
    fn test_chat_visibility_display() {
        assert_eq!(format!("{}", ChatVisibility::Full), "full");
        assert_eq!(format!("{}", ChatVisibility::System), "system");
        assert_eq!(format!("{}", ChatVisibility::Hidden), "hidden");
    }

    // ── Roundtrip id ↔ enum ─────────────────────────────────────────

    #[test]
    fn test_chat_visibility_id_roundtrip() {
        for id in 0..=2 {
            let v = ChatVisibility::by_id(id).unwrap();
            assert_eq!(v.id(), id);
        }
    }

    // ── Wire roundtrip ──────────────────────────────────────────────

    #[test]
    fn test_chat_visibility_wire_roundtrip() {
        for id in 0..=2 {
            let v = ChatVisibility::by_id(id).unwrap();
            let mut buf = BytesMut::new();
            v.write(&mut buf);
            let mut data = buf.freeze();
            let decoded = ChatVisibility::read(&mut data).unwrap();
            assert_eq!(decoded, v);
        }
    }

    #[test]
    fn test_chat_visibility_read_invalid_varint() {
        let mut data = Bytes::new();
        assert!(ChatVisibility::read(&mut data).is_err());
    }
}
