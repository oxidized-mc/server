//! Clientbound player info update packet.
//!
//! Adds or updates player entries in the tab list.
//!
//! Corresponds to `net.minecraft.network.protocol.game.ClientboundPlayerInfoUpdatePacket`.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use oxidized_auth::ProfileProperty;
use oxidized_codec::types;
use oxidized_codec::varint;

use oxidized_codec::Packet;
use oxidized_codec::packet::PacketDecodeError;

/// Action flags indicating which fields are present in each entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerInfoActions(pub u8);

impl PlayerInfoActions {
    /// Add player (name + properties).
    pub const ADD_PLAYER: u8 = 1 << 0;
    /// Initialize chat session.
    pub const INITIALIZE_CHAT: u8 = 1 << 1;
    /// Update game mode.
    pub const UPDATE_GAME_MODE: u8 = 1 << 2;
    /// Update is_listed status.
    pub const UPDATE_LISTED: u8 = 1 << 3;
    /// Update latency.
    pub const UPDATE_LATENCY: u8 = 1 << 4;
    /// Update display name.
    pub const UPDATE_DISPLAY_NAME: u8 = 1 << 5;
    /// Update list order.
    pub const UPDATE_LIST_ORDER: u8 = 1 << 6;
    /// Update hat visibility.
    pub const UPDATE_HAT: u8 = 1 << 7;

    /// Returns `true` if the given action flag is set.
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

/// A single player entry in the info update packet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerInfoEntry {
    /// Player UUID.
    pub uuid: uuid::Uuid,
    /// Player name (only with ADD_PLAYER).
    pub name: String,
    /// Profile properties (only with ADD_PLAYER).
    pub properties: Vec<ProfileProperty>,
    /// Game mode (only with UPDATE_GAME_MODE).
    pub game_mode: i32,
    /// Latency in ms (only with UPDATE_LATENCY).
    pub latency: i32,
    /// Whether is_listed in tab (only with UPDATE_LISTED).
    pub is_listed: bool,
    /// Whether player has display name (only with UPDATE_DISPLAY_NAME).
    pub has_display_name: bool,
    /// Custom display name JSON text component (only with UPDATE_DISPLAY_NAME).
    pub display_name: Option<String>,
    /// Whether to show hat model part (only with UPDATE_HAT).
    pub is_hat_visible: bool,
    /// Tab list order (only with UPDATE_LIST_ORDER).
    pub list_order: i32,
}

/// Clientbound packet that adds/updates player info entries.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundPlayerInfoUpdatePacket {
    /// Which actions (fields) are included.
    pub actions: PlayerInfoActions,
    /// Player entries.
    pub entries: Vec<PlayerInfoEntry>,
}

impl Packet for ClientboundPlayerInfoUpdatePacket {
    const PACKET_ID: i32 = 0x46;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let actions = types::read_u8(&mut data)?;
        let actions = PlayerInfoActions(actions);

        let entry_count = varint::read_varint_buf(&mut data)? as usize;
        let mut entries = Vec::with_capacity(entry_count);

        for _ in 0..entry_count {
            let uuid = types::read_uuid(&mut data)?;
            let mut entry = PlayerInfoEntry {
                uuid,
                name: String::new(),
                properties: Vec::new(),
                game_mode: 0,
                latency: 0,
                is_listed: false,
                has_display_name: false,
                display_name: None,
                is_hat_visible: false,
                list_order: 0,
            };

            if actions.contains(PlayerInfoActions::ADD_PLAYER) {
                entry.name = types::read_string(&mut data, 16)?;
                let prop_count = varint::read_varint_buf(&mut data)? as usize;
                for _ in 0..prop_count {
                    let name = types::read_string(&mut data, 32767)?;
                    let value = types::read_string(&mut data, 32767)?;
                    let has_sig = types::read_bool(&mut data)?;
                    let signature = if has_sig {
                        Some(types::read_string(&mut data, 32767)?)
                    } else {
                        None
                    };
                    entry
                        .properties
                        .push(ProfileProperty::new(name, value, signature));
                }
            }

            if actions.contains(PlayerInfoActions::INITIALIZE_CHAT) {
                let has_session = types::read_bool(&mut data)?;
                if has_session {
                    let _session_uuid = types::read_uuid(&mut data)?;
                    let _expiry = types::read_i64(&mut data)?;
                    let key_len = varint::read_varint_buf(&mut data)? as usize;
                    types::ensure_remaining(&data, key_len, "PlayerInfoUpdate session key")?;
                    data.advance(key_len);
                    let sig_len = varint::read_varint_buf(&mut data)? as usize;
                    types::ensure_remaining(&data, sig_len, "PlayerInfoUpdate session sig")?;
                    data.advance(sig_len);
                }
            }

            if actions.contains(PlayerInfoActions::UPDATE_GAME_MODE) {
                entry.game_mode = varint::read_varint_buf(&mut data)?;
            }

            if actions.contains(PlayerInfoActions::UPDATE_LISTED) {
                entry.is_listed = types::read_bool(&mut data)?;
            }

            if actions.contains(PlayerInfoActions::UPDATE_LATENCY) {
                entry.latency = varint::read_varint_buf(&mut data)?;
            }

            if actions.contains(PlayerInfoActions::UPDATE_DISPLAY_NAME) {
                entry.has_display_name = types::read_bool(&mut data)?;
                if entry.has_display_name {
                    entry.display_name = Some(types::read_string(&mut data, 32767)?);
                }
            }

            if actions.contains(PlayerInfoActions::UPDATE_LIST_ORDER) {
                entry.list_order = varint::read_varint_buf(&mut data)?;
            }

            if actions.contains(PlayerInfoActions::UPDATE_HAT) {
                entry.is_hat_visible = types::read_bool(&mut data)?;
            }

            entries.push(entry);
        }

        Ok(Self { actions, entries })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(128);
        buf.put_u8(self.actions.0);
        varint::write_varint_buf(self.entries.len() as i32, &mut buf);

        for entry in &self.entries {
            types::write_uuid(&mut buf, &entry.uuid);

            if self.actions.contains(PlayerInfoActions::ADD_PLAYER) {
                types::write_string(&mut buf, &entry.name);
                varint::write_varint_buf(entry.properties.len() as i32, &mut buf);
                for prop in &entry.properties {
                    types::write_string(&mut buf, prop.name());
                    types::write_string(&mut buf, prop.value());
                    match prop.signature() {
                        Some(sig) => {
                            types::write_bool(&mut buf, true);
                            types::write_string(&mut buf, sig);
                        },
                        None => {
                            types::write_bool(&mut buf, false);
                        },
                    }
                }
            }

            if self.actions.contains(PlayerInfoActions::INITIALIZE_CHAT) {
                types::write_bool(&mut buf, false); // No chat session
            }

            if self.actions.contains(PlayerInfoActions::UPDATE_GAME_MODE) {
                varint::write_varint_buf(entry.game_mode, &mut buf);
            }

            if self.actions.contains(PlayerInfoActions::UPDATE_LISTED) {
                types::write_bool(&mut buf, entry.is_listed);
            }

            if self.actions.contains(PlayerInfoActions::UPDATE_LATENCY) {
                varint::write_varint_buf(entry.latency, &mut buf);
            }

            if self
                .actions
                .contains(PlayerInfoActions::UPDATE_DISPLAY_NAME)
            {
                types::write_bool(&mut buf, entry.has_display_name);
                if let Some(ref display) = entry.display_name {
                    if entry.has_display_name {
                        types::write_string(&mut buf, display);
                    }
                }
            }

            if self.actions.contains(PlayerInfoActions::UPDATE_LIST_ORDER) {
                varint::write_varint_buf(entry.list_order, &mut buf);
            }

            if self.actions.contains(PlayerInfoActions::UPDATE_HAT) {
                types::write_bool(&mut buf, entry.is_hat_visible);
            }
        }

        buf
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_add_player() {
        let uuid = uuid::Uuid::new_v4();
        let pkt = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(
                PlayerInfoActions::ADD_PLAYER
                    | PlayerInfoActions::INITIALIZE_CHAT
                    | PlayerInfoActions::UPDATE_GAME_MODE
                    | PlayerInfoActions::UPDATE_LISTED
                    | PlayerInfoActions::UPDATE_LATENCY,
            ),
            entries: vec![PlayerInfoEntry {
                uuid,
                name: "TestPlayer".into(),
                properties: vec![],
                game_mode: 0,
                latency: 50,
                is_listed: true,
                has_display_name: false,
                display_name: None,
                is_hat_visible: false,
                list_order: 0,
            }],
        };

        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoUpdatePacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].uuid, uuid);
        assert_eq!(decoded.entries[0].name, "TestPlayer");
        assert_eq!(decoded.entries[0].game_mode, 0);
        assert_eq!(decoded.entries[0].latency, 50);
        assert!(decoded.entries[0].is_listed);
    }

    #[test]
    fn test_roundtrip_multiple_entries() {
        let uuid1 = uuid::Uuid::new_v4();
        let uuid2 = uuid::Uuid::new_v4();
        let pkt = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(
                PlayerInfoActions::ADD_PLAYER | PlayerInfoActions::UPDATE_GAME_MODE,
            ),
            entries: vec![
                PlayerInfoEntry {
                    uuid: uuid1,
                    name: "Alice".into(),
                    properties: vec![],
                    game_mode: 0,
                    latency: 0,
                    is_listed: false,
                    has_display_name: false,
                    display_name: None,
                    is_hat_visible: false,
                    list_order: 0,
                },
                PlayerInfoEntry {
                    uuid: uuid2,
                    name: "Bob".into(),
                    properties: vec![],
                    game_mode: 1,
                    latency: 0,
                    is_listed: false,
                    has_display_name: false,
                    display_name: None,
                    is_hat_visible: false,
                    list_order: 0,
                },
            ],
        };

        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoUpdatePacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.entries.len(), 2);
        assert_eq!(decoded.entries[0].name, "Alice");
        assert_eq!(decoded.entries[0].game_mode, 0);
        assert_eq!(decoded.entries[1].name, "Bob");
        assert_eq!(decoded.entries[1].game_mode, 1);
    }

    #[test]
    fn test_with_profile_properties() {
        let uuid = uuid::Uuid::new_v4();
        let pkt = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(PlayerInfoActions::ADD_PLAYER),
            entries: vec![PlayerInfoEntry {
                uuid,
                name: "Steve".into(),
                properties: vec![ProfileProperty::new(
                    "textures".into(),
                    "dGV4dHVyZXM=".into(),
                    Some("c2lnbmF0dXJl".into()),
                )],
                game_mode: 0,
                latency: 0,
                is_listed: false,
                has_display_name: false,
                display_name: None,
                is_hat_visible: false,
                list_order: 0,
            }],
        };

        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoUpdatePacket::decode(encoded.freeze()).unwrap();

        assert_eq!(decoded.entries[0].properties.len(), 1);
        assert_eq!(decoded.entries[0].properties[0].name(), "textures");
        assert_eq!(decoded.entries[0].properties[0].value(), "dGV4dHVyZXM=");
        assert_eq!(
            decoded.entries[0].properties[0].signature(),
            Some("c2lnbmF0dXJl")
        );
    }

    #[test]
    fn test_roundtrip_display_name() {
        let uuid = uuid::Uuid::new_v4();
        let display = r#"{"text":"Custom Name","color":"gold"}"#;
        let pkt = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(
                PlayerInfoActions::ADD_PLAYER | PlayerInfoActions::UPDATE_DISPLAY_NAME,
            ),
            entries: vec![PlayerInfoEntry {
                uuid,
                name: "Steve".into(),
                properties: vec![],
                game_mode: 0,
                latency: 0,
                is_listed: false,
                has_display_name: true,
                display_name: Some(display.into()),
                is_hat_visible: false,
                list_order: 0,
            }],
        };

        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoUpdatePacket::decode(encoded.freeze()).unwrap();

        assert!(decoded.entries[0].has_display_name);
        assert_eq!(decoded.entries[0].display_name.as_deref(), Some(display));
    }

    #[test]
    fn test_roundtrip_no_display_name() {
        let uuid = uuid::Uuid::new_v4();
        let pkt = ClientboundPlayerInfoUpdatePacket {
            actions: PlayerInfoActions(PlayerInfoActions::UPDATE_DISPLAY_NAME),
            entries: vec![PlayerInfoEntry {
                uuid,
                has_display_name: false,
                display_name: None,
                ..Default::default()
            }],
        };

        let encoded = pkt.encode();
        let decoded = ClientboundPlayerInfoUpdatePacket::decode(encoded.freeze()).unwrap();

        assert!(!decoded.entries[0].has_display_name);
        assert!(decoded.entries[0].display_name.is_none());
    }
}
