//! Clientbound login success — the server confirms the player's UUID, username,
//! and profile properties.
//!
//! Corresponds to `net.minecraft.network.protocol.login.ClientboundLoginFinishedPacket`.

use bytes::{Bytes, BytesMut};
use thiserror::Error;

use crate::codec::types::{self, TypeError};
use crate::codec::varint::{self, VarIntError};

/// Errors from decoding a [`ClientboundLoginFinishedPacket`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LoginFinishedError {
    /// VarInt decode failure.
    #[error("varint error: {0}")]
    VarInt(#[from] VarIntError),

    /// Type decode failure.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

/// A single profile property (e.g. textures) attached to a player profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProperty {
    /// The property name (e.g. `"textures"`).
    pub name: String,
    /// The base64-encoded property value.
    pub value: String,
    /// An optional base64-encoded signature from Mojang's session server.
    pub signature: Option<String>,
}

/// Clientbound packet `0x02` in the LOGIN state — login success.
///
/// Sent by the server after authentication succeeds. The client must respond
/// with a [`ServerboundLoginAcknowledgedPacket`](super::ServerboundLoginAcknowledgedPacket)
/// to transition to the CONFIGURATION state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientboundLoginFinishedPacket {
    /// The player's UUID assigned by the server.
    pub uuid: uuid::Uuid,
    /// The player's username (max 16 characters).
    pub username: String,
    /// Profile properties (e.g. skin textures).
    pub properties: Vec<ProfileProperty>,
}

impl ClientboundLoginFinishedPacket {
    /// Packet ID in the LOGIN state.
    pub const PACKET_ID: i32 = 0x02;

    /// Maximum length for property strings.
    const MAX_PROPERTY_STRING: usize = 32767;

    /// Decodes from the raw packet body.
    ///
    /// # Errors
    ///
    /// Returns [`LoginFinishedError`] if the buffer is truncated or any field
    /// is malformed.
    pub fn decode(mut data: Bytes) -> Result<Self, LoginFinishedError> {
        let uuid = types::read_uuid(&mut data)?;
        let username = types::read_string(&mut data, 16)?;

        let count = varint::read_varint_buf(&mut data)?;
        let mut properties = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let name = types::read_string(&mut data, Self::MAX_PROPERTY_STRING)?;
            let value = types::read_string(&mut data, Self::MAX_PROPERTY_STRING)?;
            let has_signature = types::read_bool(&mut data)?;
            let signature = if has_signature {
                Some(types::read_string(&mut data, Self::MAX_PROPERTY_STRING)?)
            } else {
                None
            };
            properties.push(ProfileProperty {
                name,
                value,
                signature,
            });
        }

        Ok(Self {
            uuid,
            username,
            properties,
        })
    }

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        types::write_uuid(&mut buf, &self.uuid);
        types::write_string(&mut buf, &self.username);

        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        varint::write_varint_buf(self.properties.len() as i32, &mut buf);

        for prop in &self.properties {
            types::write_string(&mut buf, &prop.name);
            types::write_string(&mut buf, &prop.value);
            types::write_bool(&mut buf, prop.signature.is_some());
            if let Some(sig) = &prop.signature {
                types::write_string(&mut buf, sig);
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
    fn test_roundtrip_with_properties() {
        let pkt = ClientboundLoginFinishedPacket {
            uuid: uuid::Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef),
            username: "Steve".to_string(),
            properties: vec![
                ProfileProperty {
                    name: "textures".to_string(),
                    value: "eyJ0ZXh0dXJlcyI6e319".to_string(),
                    signature: Some("c2lnbmF0dXJl".to_string()),
                },
                ProfileProperty {
                    name: "other".to_string(),
                    value: "dmFsdWU=".to_string(),
                    signature: None,
                },
            ],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLoginFinishedPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }

    #[test]
    fn test_roundtrip_no_properties() {
        let pkt = ClientboundLoginFinishedPacket {
            uuid: uuid::Uuid::nil(),
            username: "Alex".to_string(),
            properties: vec![],
        };
        let encoded = pkt.encode();
        let decoded = ClientboundLoginFinishedPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(decoded, pkt);
    }
}
