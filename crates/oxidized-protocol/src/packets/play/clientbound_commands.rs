//! ClientboundCommandsPacket (0x10) — sends the full command tree.
//!
//! The client uses this to build a local command graph for tab-completion
//! and syntax highlighting.

use bytes::{BufMut, BytesMut};

use crate::codec::types::write_string;
use crate::codec::varint::write_varint_buf;

/// 0x10 — Sends the full command tree to the client.
#[derive(Debug, Clone)]
pub struct ClientboundCommandsPacket {
    /// Flattened node list.
    pub nodes: Vec<CommandNodeData>,
    /// Index of the root node.
    pub root_index: i32,
}

/// A single node in the command tree wire format.
#[derive(Debug, Clone)]
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

impl ClientboundCommandsPacket {
    /// Packet ID in the PLAY state.
    pub const PACKET_ID: i32 = 0x10;

    /// Encodes the packet body (without packet ID).
    pub fn encode(&self) -> BytesMut {
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
