//! Handshaking state handler.
//!
//! Processes the initial [`ClientIntentionPacket`] and transitions the
//! connection to either [`ConnectionState::Status`] or
//! [`ConnectionState::Login`].

use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState, RawPacket};
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use tracing::{debug, warn};

/// Processes the handshake packet and transitions to the requested state.
pub async fn handle_handshake(
    conn: &mut Connection,
    pkt: RawPacket,
) -> Result<(), ConnectionError> {
    if pkt.id != ClientIntentionPacket::PACKET_ID {
        warn!(
            peer = %conn.remote_addr(),
            packet_id = pkt.id,
            "Expected handshake packet (0x00)",
        );
        return Ok(());
    }

    let intention = ClientIntentionPacket::decode(pkt.data).map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;

    conn.protocol_version = intention.protocol_version;

    debug!(
        peer = %conn.remote_addr(),
        protocol_version = intention.protocol_version,
        server_address = %intention.server_address,
        server_port = intention.server_port,
        intent = ?intention.next_state,
        "Handshake received",
    );

    conn.state = match intention.next_state {
        ClientIntent::Status => ConnectionState::Status,
        ClientIntent::Login | ClientIntent::Transfer => ConnectionState::Login,
    };

    Ok(())
}
