//! Low-precision Vec3 encoding for entity velocity in
//! [`ClientboundAddEntityPacket`].
//!
//! Mirrors `net.minecraft.network.LpVec3` in vanilla. Uses a compact
//! bit-packed format that encodes three f64 components into 6–10 bytes
//! (vs. 24 bytes for three raw doubles).
//!
//! # Wire Format
//!
//! If all components are near-zero, writes a single `0x00` byte.
//! Otherwise:
//! - 6 bytes (1 + 1 + 4): `[markers|xn_low] [xn_high|yn_low] [yn_high|zn_high|zn_low]`
//! - Optionally followed by a VarInt continuation for large scale values
//!
//! Each component is quantized to 15 bits of precision relative to a
//! shared scale factor.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use super::varint;
use super::types::TypeError;

/// Maximum absolute value that can be encoded.
const ABS_MAX_VALUE: f64 = 1.717_986_918_3e10;
/// Minimum absolute value below which the vector is treated as zero.
const ABS_MIN_VALUE: f64 = 3.051_944_088_384_301e-5;
/// Number of bits per component.
const DATA_BITS: u64 = 15;
/// Mask for extracting a component (15 bits = 2^DATA_BITS - 1).
const DATA_BITS_MASK: u64 = (1 << DATA_BITS) - 1;
/// Maximum quantized value.
const MAX_QUANTIZED_VALUE: f64 = 32766.0;
/// Bit offset for X component in the packed buffer.
const X_OFFSET: u64 = 3;
/// Bit offset for Y component.
const Y_OFFSET: u64 = 18;
/// Bit offset for Z component.
const Z_OFFSET: u64 = 33;
/// Mask for scale bits in the lowest byte.
const SCALE_BITS_MASK: u64 = 3;
/// Flag indicating a VarInt continuation follows.
const CONTINUATION_FLAG: u64 = 4;

/// Sanitizes a component value: clamps to range, replaces NaN with 0.
fn sanitize(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-ABS_MAX_VALUE, ABS_MAX_VALUE)
    }
}

/// Quantizes a normalized component (-1..1) to a 15-bit integer.
fn pack(value: f64) -> u64 {
    ((value * 0.5 + 0.5) * MAX_QUANTIZED_VALUE).round() as u64
}

/// Dequantizes a 15-bit integer to a normalized component (-1..1).
fn unpack(value: u64) -> f64 {
    (value & DATA_BITS_MASK).min(MAX_QUANTIZED_VALUE as u64) as f64 * 2.0
        / MAX_QUANTIZED_VALUE
        - 1.0
}

/// Returns the maximum of three absolute values.
fn abs_max3(a: f64, b: f64, c: f64) -> f64 {
    a.abs().max(b.abs()).max(c.abs())
}

/// Encodes a velocity vector into the LpVec3 wire format.
///
/// # Examples
///
/// ```
/// use oxidized_protocol::codec::lp_vec3;
///
/// let mut buf = bytes::BytesMut::new();
/// lp_vec3::write(&mut buf, 0.0, 0.0, 0.0);
/// assert_eq!(buf.as_ref(), &[0x00]); // zero vector = single byte
/// ```
pub fn write(buf: &mut BytesMut, x: f64, y: f64, z: f64) {
    let x = sanitize(x);
    let y = sanitize(y);
    let z = sanitize(z);

    let chessboard_length = abs_max3(x, y, z);
    if chessboard_length < ABS_MIN_VALUE {
        buf.put_u8(0);
        return;
    }

    let scale = ceil_long(chessboard_length);
    let is_partial = (scale & 3) != scale;
    let markers = if is_partial {
        (scale & 3) | 4
    } else {
        scale
    };

    let xn = pack(x / scale as f64) << X_OFFSET;
    let yn = pack(y / scale as f64) << Y_OFFSET;
    let zn = pack(z / scale as f64) << Z_OFFSET;
    let buffer = markers as u64 | xn | yn | zn;

    buf.put_u8(buffer as u8);
    buf.put_u8((buffer >> 8) as u8);
    buf.put_u32((buffer >> 16) as u32);

    if is_partial {
        varint::write_varint_buf((scale >> 2) as i32, buf);
    }
}

/// Decodes a velocity vector from the LpVec3 wire format.
///
/// Returns `(x, y, z)` as f64 velocities.
///
/// # Errors
///
/// Returns [`TypeError::UnexpectedEof`] if the buffer is too short.
pub fn read(buf: &mut Bytes) -> Result<(f64, f64, f64), TypeError> {
    if buf.remaining() < 1 {
        return Err(TypeError::UnexpectedEof {
            need: 1,
            have: 0,
        });
    }

    let lowest = buf.get_u8() as u64;
    if lowest == 0 {
        return Ok((0.0, 0.0, 0.0));
    }

    if buf.remaining() < 5 {
        return Err(TypeError::UnexpectedEof {
            need: 5,
            have: buf.remaining(),
        });
    }

    let middle = buf.get_u8() as u64;
    let highest = buf.get_u32() as u64;
    let buffer = highest << 16 | middle << 8 | lowest;

    let mut scale = lowest & SCALE_BITS_MASK;
    if has_continuation_bit(lowest) {
        let continuation = varint::read_varint_buf(buf)? as u64;
        scale |= continuation << 2;
    }

    let x = unpack(buffer >> X_OFFSET) * scale as f64;
    let y = unpack(buffer >> Y_OFFSET) * scale as f64;
    let z = unpack(buffer >> Z_OFFSET) * scale as f64;

    Ok((x, y, z))
}

/// Checks the continuation flag bit.
fn has_continuation_bit(lowest: u64) -> bool {
    (lowest & CONTINUATION_FLAG) == CONTINUATION_FLAG
}

/// Ceiling of a positive float as i64 (matches Java `Mth.ceilLong`).
fn ceil_long(value: f64) -> i64 {
    value.ceil() as i64
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn roundtrip(x: f64, y: f64, z: f64, tolerance: f64) {
        let mut buf = BytesMut::new();
        write(&mut buf, x, y, z);
        let mut data = buf.freeze();
        let (rx, ry, rz) = read(&mut data).unwrap();
        assert!(
            (rx - x).abs() < tolerance,
            "x: expected {x}, got {rx}"
        );
        assert!(
            (ry - y).abs() < tolerance,
            "y: expected {y}, got {ry}"
        );
        assert!(
            (rz - z).abs() < tolerance,
            "z: expected {z}, got {rz}"
        );
    }

    #[test]
    fn test_zero_vector() {
        let mut buf = BytesMut::new();
        write(&mut buf, 0.0, 0.0, 0.0);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0], 0);

        let mut data = buf.freeze();
        let (x, y, z) = read(&mut data).unwrap();
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_small_velocity_roundtrip() {
        // Typical entity velocity ~0.1 blocks/tick
        roundtrip(0.1, -0.08, 0.0, 0.01);
    }

    #[test]
    fn test_unit_velocity_roundtrip() {
        roundtrip(1.0, 0.0, 0.0, 0.01);
    }

    #[test]
    fn test_negative_velocity_roundtrip() {
        roundtrip(-1.0, -1.0, -1.0, 0.01);
    }

    #[test]
    fn test_near_zero_treated_as_zero() {
        let mut buf = BytesMut::new();
        write(&mut buf, 1e-6, 1e-6, 1e-6);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn test_nan_sanitized_to_zero() {
        let mut buf = BytesMut::new();
        write(&mut buf, f64::NAN, 0.0, 0.0);
        assert_eq!(buf[0], 0); // all zero
    }

    #[test]
    fn test_large_velocity_roundtrip() {
        // Large but within range
        roundtrip(100.0, -50.0, 75.0, 1.0);
    }

    #[test]
    fn test_read_empty_buffer_errors() {
        let mut data = Bytes::new();
        assert!(read(&mut data).is_err());
    }
}
