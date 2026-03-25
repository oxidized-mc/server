//! ClientboundCommandsPacket (0x10) — sends the full command tree.
//!
//! The client uses this to build a local command graph for tab-completion
//! and syntax highlighting.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::codec::packet::{Packet, PacketDecodeError};
use crate::codec::types::{ensure_remaining, read_string, write_string};
use crate::codec::varint::{read_varint_buf, write_varint_buf};

/// Maximum string length in the Minecraft protocol (characters).
const MAX_STRING_LENGTH: usize = 32767;

/// 0x10 — Sends the full command tree to the client.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientboundCommandsPacket {
    /// Flattened node list.
    pub nodes: Vec<CommandNodeData>,
    /// Index of the root node.
    pub root_index: i32,
}

/// A single node in the command tree wire format.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandNodeData {
    /// Packed flags (type, executable, redirect, suggestions).
    pub flags: u8,
    /// Child node indices.
    pub children: Vec<i32>,
    /// Redirect target index.
    pub redirect_node: Option<i32>,
    /// Node name (literal or argument name; absent for root).
    pub name: Option<String>,
    /// Parser ID and properties (argument nodes only).
    pub parser_id: Option<i32>,
    /// Serialized parser properties.
    pub parser_properties: Option<Vec<u8>>,
    /// Custom suggestions type identifier.
    pub suggestions_type: Option<String>,
}

impl Packet for ClientboundCommandsPacket {
    const PACKET_ID: i32 = 0x10;

    fn decode(mut data: Bytes) -> Result<Self, PacketDecodeError> {
        let node_count = read_varint_buf(&mut data)?;
        if node_count < 0 {
            return Err(PacketDecodeError::InvalidData(format!(
                "negative node count: {node_count}"
            )));
        }

        let mut nodes = Vec::with_capacity(node_count as usize);
        for _ in 0..node_count {
            nodes.push(decode_node(&mut data)?);
        }

        let root_index = read_varint_buf(&mut data)?;

        Ok(Self { nodes, root_index })
    }

    fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(4096);

        // Node count
        write_varint_buf(self.nodes.len() as i32, &mut buf);

        for node in &self.nodes {
            // Flags
            buf.put_u8(node.flags);

            // Children count + indices
            write_varint_buf(node.children.len() as i32, &mut buf);
            for &child in &node.children {
                write_varint_buf(child, &mut buf);
            }

            // Redirect target (if FLAG_REDIRECT is set, bit 3)
            if node.flags & 0x08 != 0 {
                if let Some(redirect) = node.redirect_node {
                    write_varint_buf(redirect, &mut buf);
                }
            }

            let node_type = node.flags & 0x03;

            // Literal name (type 1)
            if node_type == 1 {
                if let Some(ref name) = node.name {
                    write_string(&mut buf, name);
                }
            }

            // Argument name + parser (type 2)
            if node_type == 2 {
                if let Some(ref name) = node.name {
                    write_string(&mut buf, name);
                }
                if let Some(parser_id) = node.parser_id {
                    write_varint_buf(parser_id, &mut buf);
                }
                if let Some(ref props) = node.parser_properties {
                    buf.extend_from_slice(props);
                }
                // Custom suggestions type (if FLAG_SUGGESTIONS is set, bit 4)
                if node.flags & 0x10 != 0 {
                    if let Some(ref st) = node.suggestions_type {
                        write_string(&mut buf, st);
                    }
                }
            }
        }

        // Root index
        write_varint_buf(self.root_index, &mut buf);

        buf
    }
}

/// Decodes a single command node from the wire format.
fn decode_node(buf: &mut Bytes) -> Result<CommandNodeData, PacketDecodeError> {
    if buf.remaining() < 1 {
        return Err(PacketDecodeError::InvalidData(
            "truncated node flags".into(),
        ));
    }
    let flags = buf.get_u8();

    // Children
    let child_count = read_varint_buf(buf)?;
    if child_count < 0 {
        return Err(PacketDecodeError::InvalidData(format!(
            "negative child count: {child_count}"
        )));
    }
    let mut children = Vec::with_capacity(child_count as usize);
    for _ in 0..child_count {
        children.push(read_varint_buf(buf)?);
    }

    // Redirect target (bit 3)
    let redirect_node = if flags & 0x08 != 0 {
        Some(read_varint_buf(buf)?)
    } else {
        None
    };

    let node_type = flags & 0x03;

    // Name (literal = type 1, argument = type 2)
    let name = if node_type == 1 || node_type == 2 {
        Some(read_string(buf, MAX_STRING_LENGTH)?)
    } else {
        None
    };

    // Parser info (argument = type 2 only)
    let (parser_id, parser_properties, suggestions_type) = if node_type == 2 {
        let pid = read_varint_buf(buf)?;
        let props = read_parser_properties(pid, buf)?;

        let stype = if flags & 0x10 != 0 {
            Some(read_string(buf, MAX_STRING_LENGTH)?)
        } else {
            None
        };

        (Some(pid), Some(props), stype)
    } else {
        (None, None, None)
    };

    Ok(CommandNodeData {
        flags,
        children,
        redirect_node,
        name,
        parser_id,
        parser_properties,
        suggestions_type,
    })
}

/// Reads parser-specific property bytes for the given `parser_id`.
///
/// Each of the 57 argument types has a known wire layout. This function
/// advances `buf` past the properties and returns the consumed bytes as an
/// opaque vector suitable for re-encoding with `extend_from_slice`.
///
/// # Errors
///
/// Returns [`PacketDecodeError::InvalidData`] for unknown parser IDs or
/// truncated buffers.
fn read_parser_properties(parser_id: i32, buf: &mut Bytes) -> Result<Vec<u8>, PacketDecodeError> {
    let before = buf.clone();

    match parser_id {
        // brigadier:bool — no properties
        0 => {},
        // brigadier:float (1), brigadier:integer (3) — flags + optional 4-byte min/max
        1 | 3 => {
            let value_size = 4;
            skip_number_parser(buf, value_size)?;
        },
        // brigadier:double (2), brigadier:long (4) — flags + optional 8-byte min/max
        2 | 4 => {
            let value_size = 8;
            skip_number_parser(buf, value_size)?;
        },
        // brigadier:string — 1 VarInt (StringType enum)
        5 => {
            read_varint_buf(buf)?;
        },
        // minecraft:entity — 1 byte flags
        6 => {
            ensure_remaining(buf, 1, "entity parser flags")?;
            buf.advance(1);
        },
        // minecraft:game_profile through minecraft:nbt_path — no properties
        7..=29 => {},
        // minecraft:score_holder — 1 byte flags
        30 => {
            ensure_remaining(buf, 1, "score_holder parser flags")?;
            buf.advance(1);
        },
        // minecraft:swizzle through minecraft:gamemode — no properties
        31..=41 => {},
        // minecraft:time — 1 i32 (minimum)
        42 => {
            ensure_remaining(buf, 4, "time parser minimum")?;
            buf.advance(4);
        },
        // resource/tag variants — 1 ResourceLocation (VarInt-prefixed string)
        43..=46 => {
            read_string(buf, MAX_STRING_LENGTH)?;
        },
        // minecraft:template_mirror through minecraft:enchantable_slot — no properties
        47..=56 => {},
        _ => {
            return Err(PacketDecodeError::InvalidData(format!(
                "unknown parser ID: {parser_id}"
            )));
        },
    }

    let consumed = before.remaining() - buf.remaining();
    Ok(before[..consumed].to_vec())
}

/// Skips a numeric parser's flags byte and optional min/max values.
///
/// `value_size` is 4 for `f32`/`i32` and 8 for `f64`/`i64`.
fn skip_number_parser(buf: &mut Bytes, value_size: usize) -> Result<(), PacketDecodeError> {
    ensure_remaining(buf, 1, "number parser flags")?;
    let flags = buf.get_u8();
    if flags & 0x01 != 0 {
        ensure_remaining(buf, value_size, "number parser min")?;
        buf.advance(value_size);
    }
    if flags & 0x02 != 0 {
        ensure_remaining(buf, value_size, "number parser max")?;
        buf.advance(value_size);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::codec::packet::Packet;

    /// Helper to build a root node (type 0).
    fn root_node(children: Vec<i32>) -> CommandNodeData {
        CommandNodeData {
            flags: 0x00,
            children,
            redirect_node: None,
            name: None,
            parser_id: None,
            parser_properties: None,
            suggestions_type: None,
        }
    }

    /// Helper to build a literal node (type 1).
    fn literal_node(name: &str, flags_extra: u8, children: Vec<i32>) -> CommandNodeData {
        CommandNodeData {
            flags: 0x01 | flags_extra,
            children,
            redirect_node: None,
            name: Some(name.to_string()),
            parser_id: None,
            parser_properties: None,
            suggestions_type: None,
        }
    }

    #[test]
    fn test_commands_packet_roundtrip_root_only() {
        let pkt = ClientboundCommandsPacket {
            nodes: vec![root_node(vec![])],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_literal() {
        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                literal_node("test", 0x04, vec![]), // executable
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_argument_bool() {
        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04, // argument + executable
                    children: vec![],
                    redirect_node: None,
                    name: Some("target".to_string()),
                    parser_id: Some(0), // brigadier:bool — no properties
                    parser_properties: Some(vec![]),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_with_redirect() {
        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x01 | 0x08, // literal + redirect
                    children: vec![],
                    redirect_node: Some(0),
                    name: Some("loop".to_string()),
                    parser_id: None,
                    parser_properties: None,
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_float_parser() {
        // brigadier:float with min=0.0 and max=100.0
        // flags=0x03, min=0.0f32 BE, max=100.0f32 BE
        let mut props = vec![0x03u8]; // HAS_MIN | HAS_MAX
        props.extend_from_slice(&0.0f32.to_be_bytes());
        props.extend_from_slice(&100.0f32.to_be_bytes());

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("amount".to_string()),
                    parser_id: Some(1), // brigadier:float
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_integer_min_only() {
        // brigadier:integer with min=0 only
        let mut props = vec![0x01u8]; // HAS_MIN
        props.extend_from_slice(&0i32.to_be_bytes());

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("count".to_string()),
                    parser_id: Some(3), // brigadier:integer
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_double_parser() {
        // brigadier:double with max=1.0 only
        let mut props = vec![0x02u8]; // HAS_MAX
        props.extend_from_slice(&1.0f64.to_be_bytes());

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("scale".to_string()),
                    parser_id: Some(2), // brigadier:double
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_string_parser() {
        // brigadier:string with QUOTABLE_PHRASE (enum value 1)
        let props = vec![0x01u8]; // VarInt(1)

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("msg".to_string()),
                    parser_id: Some(5), // brigadier:string
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_entity_parser() {
        // minecraft:entity — 1 byte flags (0x01 = single entity only)
        let props = vec![0x01u8];

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("target".to_string()),
                    parser_id: Some(6), // minecraft:entity
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_time_parser() {
        // minecraft:time — 4 bytes i32 minimum (0)
        let props = 0i32.to_be_bytes().to_vec();

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("duration".to_string()),
                    parser_id: Some(42), // minecraft:time
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_resource_parser() {
        // minecraft:resource — ResourceLocation as VarInt-prefixed string
        let mut props = BytesMut::new();
        write_string(&mut props, "minecraft:entity_type");
        let props = props.to_vec();

        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04,
                    children: vec![],
                    redirect_node: None,
                    name: Some("entity".to_string()),
                    parser_id: Some(45), // minecraft:resource
                    parser_properties: Some(props),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_with_suggestions() {
        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                CommandNodeData {
                    flags: 0x02 | 0x04 | 0x10, // argument + executable + suggestions
                    children: vec![],
                    redirect_node: None,
                    name: Some("target".to_string()),
                    parser_id: Some(0), // brigadier:bool
                    parser_properties: Some(vec![]),
                    suggestions_type: Some("minecraft:ask_server".to_string()),
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }

    #[test]
    fn test_commands_packet_roundtrip_complex_tree() {
        // Root → "tp" (literal) → "target" (entity arg) → "destination" (vec3 arg)
        let pkt = ClientboundCommandsPacket {
            nodes: vec![
                root_node(vec![1]),
                literal_node("tp", 0x00, vec![2]),
                CommandNodeData {
                    flags: 0x02, // argument
                    children: vec![3],
                    redirect_node: None,
                    name: Some("target".to_string()),
                    parser_id: Some(6), // minecraft:entity
                    parser_properties: Some(vec![0x01]),
                    suggestions_type: None,
                },
                CommandNodeData {
                    flags: 0x02 | 0x04, // argument + executable
                    children: vec![],
                    redirect_node: None,
                    name: Some("destination".to_string()),
                    parser_id: Some(10), // minecraft:vec3 — no properties
                    parser_properties: Some(vec![]),
                    suggestions_type: None,
                },
            ],
            root_index: 0,
        };
        let encoded = pkt.encode();
        let decoded = ClientboundCommandsPacket::decode(encoded.freeze()).unwrap();
        assert_eq!(pkt, decoded);
    }
}
