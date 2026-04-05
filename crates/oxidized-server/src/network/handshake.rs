//! Handshaking state handler.
//!
//! Processes the initial [`ClientIntentionPacket`] and transitions the
//! connection to either [`ConnectionState::Status`] or
//! [`ConnectionState::Login`].

use oxidized_codec::Packet;
use oxidized_protocol::constants::VERSION_NAME;
use oxidized_protocol::packets::handshake::{ClientIntent, ClientIntentionPacket};
use oxidized_transport::connection::{
    Connection, ConnectionError, ConnectionState, RawPacket,
};
use tracing::{debug, warn};

use super::helpers::{decode_packet, disconnect_err};

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

    let intention: ClientIntentionPacket =
        decode_packet(pkt.data, conn.remote_addr(), "", "Handshake")?;

    conn.protocol_version = intention.protocol_version;

    // Validate protocol version for login connections (not status pings).
    if matches!(
        intention.next_state,
        ClientIntent::Login | ClientIntent::Transfer
    ) && intention.protocol_version != oxidized_protocol::constants::PROTOCOL_VERSION
    {
        warn!(
            peer = %conn.remote_addr(),
            client_version = intention.protocol_version,
            server_version = oxidized_protocol::constants::PROTOCOL_VERSION,
            "Protocol version mismatch — disconnecting",
        );

        // Transition to Login state so we can send the disconnect packet
        // (vanilla: `this.connection.setupOutboundProtocol(LoginProtocols.CLIENTBOUND)`).
        conn.state = ConnectionState::Login;

        // Vanilla uses "outdated_client" for very old protocols (< 754) and
        // "incompatible" for everything else.
        let reason = if intention.protocol_version < 754 {
            format!(
                r#"{{"translate":"multiplayer.disconnect.outdated_client","with":["{VERSION_NAME}"]}}"#
            )
        } else {
            format!(
                r#"{{"translate":"multiplayer.disconnect.incompatible","with":["{VERSION_NAME}"]}}"#
            )
        };

        return Err(disconnect_err(conn, &reason).await);
    }

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
