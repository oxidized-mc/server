//! Packet frame codec for the Minecraft wire protocol.
//!
//! Each packet is framed as `VarInt(length) || payload[length]`.
//! The length prefix encodes the combined size of the packet ID (VarInt)
//! and the packet body. This module handles reading and writing these frames.

use std::io;

use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::varint::{self, VarIntError};

/// Maximum packet size (2 MiB) — vanilla default.
pub const MAX_PACKET_SIZE: usize = 2 * 1024 * 1024;

/// Errors that can occur during frame decoding.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FrameError {
    /// The packet exceeds the maximum allowed size.
    #[error("packet too large: {size} bytes (max {max})")]
    PacketTooLarge {
        /// Actual size declared by the length prefix.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// Zero-length frame (no packet ID present).
    #[error("zero-length frame")]
    ZeroLength,

    /// A VarInt decoding error occurred.
    #[error("varint error: {0}")]
    VarInt(#[from] VarIntError),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Reads one packet frame from an async reader.
///
/// Returns the raw frame payload (packet ID + packet body, without the
/// length prefix).
///
/// # Errors
///
/// Returns [`FrameError`] if the length prefix is invalid, the packet is
/// too large, or an I/O error occurs.
pub async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    max_size: usize,
) -> Result<Bytes, FrameError> {
    let length = varint::read_varint_async(reader)
        .await
        .map_err(|e| match e.kind() {
            io::ErrorKind::InvalidData => FrameError::VarInt(VarIntError::TooLarge {
                max_bytes: varint::VARINT_MAX_BYTES,
            }),
            _ => FrameError::Io(e),
        })?;

    let length = length as usize;
    if length == 0 {
        return Err(FrameError::ZeroLength);
    }
    if length > max_size {
        return Err(FrameError::PacketTooLarge {
            size: length,
            max: max_size,
        });
    }

    let mut buf = BytesMut::zeroed(length);
    reader.read_exact(&mut buf).await?;
    Ok(buf.freeze())
}

/// Writes one packet frame to an async writer.
///
/// Encodes the data length as a VarInt prefix followed by the raw payload.
///
/// # Errors
///
/// Returns [`io::Error`] on write failure.
pub async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    data: &[u8],
) -> Result<(), io::Error> {
    varint::write_varint_async(writer, data.len() as i32).await?;
    writer.write_all(data).await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_snapshots() {
        insta::assert_snapshot!(
            "packet_too_large",
            format!(
                "{}",
                FrameError::PacketTooLarge {
                    size: 3_000_000,
                    max: 2_097_152,
                }
            )
        );
        insta::assert_snapshot!("zero_length", format!("{}", FrameError::ZeroLength));
    }

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let payload = b"\x00Hello, Minecraft!";
        let mut wire = Vec::new();
        write_frame(&mut wire, payload).await.unwrap();

        let mut cursor = io::Cursor::new(wire);
        let decoded = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap();
        assert_eq!(&decoded[..], payload);
    }

    #[tokio::test]
    async fn test_frame_roundtrip_large_payload() {
        let payload = vec![0xABu8; 4096];
        let mut wire = Vec::new();
        write_frame(&mut wire, &payload).await.unwrap();

        let mut cursor = io::Cursor::new(wire);
        let decoded = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap();
        assert_eq!(&decoded[..], &payload[..]);
    }

    #[tokio::test]
    async fn test_frame_too_large() {
        // Manually encode a length of MAX_PACKET_SIZE + 1
        let mut wire = Vec::new();
        varint::write_varint_async(&mut wire, (MAX_PACKET_SIZE + 1) as i32)
            .await
            .unwrap();

        let mut cursor = io::Cursor::new(wire);
        let err = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap_err();
        assert!(matches!(err, FrameError::PacketTooLarge { .. }));
    }

    #[tokio::test]
    async fn test_frame_zero_length() {
        // Encode a length of 0
        let mut wire = Vec::new();
        varint::write_varint_async(&mut wire, 0).await.unwrap();

        let mut cursor = io::Cursor::new(wire);
        let err = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap_err();
        assert!(matches!(err, FrameError::ZeroLength));
    }

    #[tokio::test]
    async fn test_frame_eof_mid_payload() {
        // Write length of 100 but only 10 bytes of data
        let mut wire = Vec::new();
        varint::write_varint_async(&mut wire, 100).await.unwrap();
        wire.extend_from_slice(&[0u8; 10]);

        let mut cursor = io::Cursor::new(wire);
        let err = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap_err();
        assert!(matches!(err, FrameError::Io(_)));
    }

    #[tokio::test]
    async fn test_frame_multiple_sequential() {
        let payloads: Vec<&[u8]> = vec![b"\x00ping", b"\x01pong", b"\x02data"];
        let mut wire = Vec::new();
        for payload in &payloads {
            write_frame(&mut wire, payload).await.unwrap();
        }

        let mut cursor = io::Cursor::new(wire);
        for expected in &payloads {
            let decoded = read_frame(&mut cursor, MAX_PACKET_SIZE).await.unwrap();
            assert_eq!(&decoded[..], *expected);
        }
    }
}
