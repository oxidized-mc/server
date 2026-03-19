//! Configuration state handler.
//!
//! Sends registry data, tags, enabled features, and transitions the
//! client to PLAY state. The configuration flow is server-driven.

use oxidized_protocol::codec::Packet;
use oxidized_protocol::connection::{Connection, ConnectionError, ConnectionState};
use oxidized_protocol::packets::configuration::{
    ClientInformation, ClientboundFinishConfigurationPacket, ClientboundRegistryDataPacket,
    ClientboundSelectKnownPacksPacket, ClientboundUpdateEnabledFeaturesPacket,
    ClientboundUpdateTagsPacket, KnownPack, RegistryEntry, ServerboundClientInformationPacket,
    ServerboundFinishConfigurationPacket, ServerboundSelectKnownPacksPacket,
};
use oxidized_protocol::registry;
use oxidized_protocol::types::resource_location::ResourceLocation;
use tracing::{debug, info, warn};

use super::helpers::{decode_packet, disconnect_err};

/// Handles the CONFIGURATION state — sends registry data, tags, features,
/// and transitions the client to PLAY.
///
/// The configuration flow is server-driven:
/// 1. Send `ClientboundSelectKnownPacksPacket`
/// 2. Receive `ServerboundSelectKnownPacksPacket`
/// 3. Send `ClientboundRegistryDataPacket` × N (one per synchronized registry)
/// 4. Send `ClientboundUpdateTagsPacket` (all tag registries with entries)
/// 5. Send `ClientboundUpdateEnabledFeaturesPacket` (vanilla features)
/// 6. Send `ClientboundFinishConfigurationPacket`
/// 7. Receive `ServerboundFinishConfigurationPacket`
/// 8. Transition to PLAY state
///
/// Returns the [`ClientInformation`] received from the client (language,
/// view distance, etc.) for use in player setup.
///
/// # Errors
///
/// Returns a [`ConnectionError`] if any I/O, decoding, or protocol step fails.
pub async fn handle_configuration(
    conn: &mut Connection,
) -> Result<ClientInformation, ConnectionError> {
    let addr = conn.remote_addr();
    let mut client_info: Option<ClientInformation> = None;

    // 1. Send SelectKnownPacks — we claim to have the vanilla core pack
    let known_packs = ClientboundSelectKnownPacksPacket {
        packs: vec![KnownPack {
            namespace: "minecraft".to_string(),
            id: "core".to_string(),
            version: "1.21.6".to_string(),
        }],
    };
    conn.send_packet(&known_packs).await?;
    debug!(peer = %addr, "Sent SelectKnownPacks");

    // 2. Receive serverbound packets until we get SelectKnownPacks.
    //    The client may send ClientInformation (0x00) or CustomPayload
    //    (0x02, e.g. minecraft:brand) before responding.
    const SB_CUSTOM_PAYLOAD: i32 = 0x02;
    loop {
        let pkt = conn.read_raw_packet().await?;
        match pkt.id {
            ServerboundClientInformationPacket::PACKET_ID => {
                let info_pkt: ServerboundClientInformationPacket =
                    decode_packet(pkt.data, addr, "", "ClientInformation")?;
                debug!(
                    peer = %addr,
                    language = %info_pkt.information.language,
                    view_distance = info_pkt.information.view_distance,
                    "Received client information",
                );
                client_info = Some(info_pkt.information);
            },
            SB_CUSTOM_PAYLOAD => {
                debug!(peer = %addr, "Received custom payload (ignored)");
            },
            ServerboundSelectKnownPacksPacket::PACKET_ID => {
                let _client_packs: ServerboundSelectKnownPacksPacket =
                    decode_packet(pkt.data, addr, "", "SelectKnownPacks")?;
                debug!(peer = %addr, "Received client known packs response");
                break;
            },
            _ => {
                warn!(peer = %addr, id = pkt.id, "Unexpected packet during configuration");
                return Err(disconnect_err(conn, "Unexpected packet during configuration").await);
            },
        }
    }

    // 3. Send all synchronized registries (full data, ignoring known-pack
    //    optimisation for now)
    for registry_name in registry::SYNCHRONIZED_REGISTRIES {
        let entries = registry::get_registry_entries(registry_name).map_err(|e| {
            ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        let registry_loc = ResourceLocation::from_string(registry_name).map_err(|e| {
            ConnectionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        let reg_entries: Vec<RegistryEntry> = entries
            .into_iter()
            .map(|(name, compound)| {
                let id = ResourceLocation::from_string(&name).map_err(|e| {
                    ConnectionError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    ))
                })?;
                Ok(RegistryEntry {
                    id,
                    data: Some(compound),
                })
            })
            .collect::<Result<Vec<_>, ConnectionError>>()?;

        let packet = ClientboundRegistryDataPacket {
            registry: registry_loc,
            entries: reg_entries,
        };

        // Use send_raw without flush here — we batch all registries and
        // flush once after the loop.
        let body = packet.encode();
        conn.send_raw(ClientboundRegistryDataPacket::PACKET_ID, &body)
            .await?;
    }
    conn.flush().await?;
    debug!(
        peer = %addr,
        count = registry::SYNCHRONIZED_REGISTRIES.len(),
        "Sent all registry data",
    );

    // 4. Send tags (block, item, fluid, entity_type, enchantment, etc.)
    let tags_packet = registry::build_tags_packet();
    let tag_count = tags_packet.tags.len();
    conn.send_packet(&tags_packet).await?;
    debug!(peer = %addr, registries = tag_count, "Sent tags");

    // 5. Send enabled features (vanilla feature set)
    let vanilla_feature = ResourceLocation::from_string("minecraft:vanilla").map_err(|e| {
        ConnectionError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;
    let features_packet = ClientboundUpdateEnabledFeaturesPacket {
        features: vec![vanilla_feature],
    };
    conn.send_packet(&features_packet).await?;
    debug!(peer = %addr, "Sent enabled features");

    // 6. Send FinishConfiguration
    let finish = ClientboundFinishConfigurationPacket;
    conn.send_packet(&finish).await?;
    debug!(peer = %addr, "Sent finish configuration");

    // 7. Wait for client FinishConfiguration acknowledgement.
    //    The client may send ClientInformation or CustomPayload again.
    loop {
        let finish_pkt = conn.read_raw_packet().await?;
        match finish_pkt.id {
            ServerboundClientInformationPacket::PACKET_ID => {
                let info_pkt: ServerboundClientInformationPacket =
                    decode_packet(finish_pkt.data, addr, "", "ClientInformation")?;
                debug!(
                    peer = %addr,
                    language = %info_pkt.information.language,
                    view_distance = info_pkt.information.view_distance,
                    "Received updated client information",
                );
                client_info = Some(info_pkt.information);
            },
            SB_CUSTOM_PAYLOAD => {
                debug!(peer = %addr, "Received custom payload (ignored)");
            },
            ServerboundFinishConfigurationPacket::PACKET_ID => {
                break;
            },
            _ => {
                warn!(peer = %addr, id = finish_pkt.id, "Expected FinishConfiguration");
                return Err(disconnect_err(
                    conn,
                    "Unexpected packet — expected finish configuration",
                )
                .await);
            },
        }
    }

    // Use client_info (or defaults) for this session
    let client_info = client_info.unwrap_or_else(ClientInformation::create_default);

    // 8. Transition to Play
    conn.state = ConnectionState::Play;
    info!(peer = %addr, "Configuration complete — client entering PLAY state");

    Ok(client_info)
}
