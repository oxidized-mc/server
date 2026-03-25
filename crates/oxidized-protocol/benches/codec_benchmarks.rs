//! Benchmarks for oxidized-protocol: VarInt/VarLong encode/decode,
//! packet encode/decode.
#![allow(missing_docs, clippy::unwrap_used)]

use bytes::{Bytes, BytesMut};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidized_protocol::codec::varint::{
    VARINT_MAX_BYTES, VARLONG_MAX_BYTES, decode_varint, decode_varlong, encode_varint,
    encode_varlong, read_varint_buf, varint_size, write_varint_buf,
};

// -----------------------------------------------------------------------
// VarInt encode / decode
// -----------------------------------------------------------------------

fn bench_varint_encode_small(c: &mut Criterion) {
    c.bench_function("varint_encode_1byte", |b| {
        b.iter(|| {
            let mut buf = [0u8; VARINT_MAX_BYTES];
            black_box(encode_varint(black_box(42), &mut buf));
        });
    });
}

fn bench_varint_encode_large(c: &mut Criterion) {
    c.bench_function("varint_encode_5byte", |b| {
        b.iter(|| {
            let mut buf = [0u8; VARINT_MAX_BYTES];
            black_box(encode_varint(black_box(-1), &mut buf));
        });
    });
}

fn bench_varint_decode_small(c: &mut Criterion) {
    let mut buf = [0u8; VARINT_MAX_BYTES];
    let len = encode_varint(42, &mut buf);
    let data = &buf[..len];

    c.bench_function("varint_decode_1byte", |b| {
        b.iter(|| {
            black_box(decode_varint(black_box(data))).ok();
        });
    });
}

fn bench_varint_decode_large(c: &mut Criterion) {
    let mut buf = [0u8; VARINT_MAX_BYTES];
    let len = encode_varint(-1, &mut buf);
    let data = &buf[..len];

    c.bench_function("varint_decode_5byte", |b| {
        b.iter(|| {
            black_box(decode_varint(black_box(data))).ok();
        });
    });
}

fn bench_varint_roundtrip(c: &mut Criterion) {
    let values = [0, 1, 127, 128, 255, 25565, 2_097_151, i32::MAX, -1, i32::MIN];

    c.bench_function("varint_roundtrip_10_values", |b| {
        b.iter(|| {
            for &v in &values {
                let mut buf = [0u8; VARINT_MAX_BYTES];
                let len = encode_varint(v, &mut buf);
                let (decoded, _) = decode_varint(&buf[..len]).ok().unwrap();
                black_box(decoded);
            }
        });
    });
}

// -----------------------------------------------------------------------
// VarLong encode / decode
// -----------------------------------------------------------------------

fn bench_varlong_encode(c: &mut Criterion) {
    c.bench_function("varlong_encode_10byte", |b| {
        b.iter(|| {
            let mut buf = [0u8; VARLONG_MAX_BYTES];
            black_box(encode_varlong(black_box(-1i64), &mut buf));
        });
    });
}

fn bench_varlong_decode(c: &mut Criterion) {
    let mut buf = [0u8; VARLONG_MAX_BYTES];
    let len = encode_varlong(-1i64, &mut buf);
    let data = &buf[..len];

    c.bench_function("varlong_decode_10byte", |b| {
        b.iter(|| {
            black_box(decode_varlong(black_box(data))).ok();
        });
    });
}

// -----------------------------------------------------------------------
// BytesMut helpers
// -----------------------------------------------------------------------

fn bench_varint_buf_write(c: &mut Criterion) {
    c.bench_function("varint_buf_write", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(5);
            write_varint_buf(black_box(25565), &mut buf);
            black_box(buf);
        });
    });
}

fn bench_varint_buf_read(c: &mut Criterion) {
    let mut buf = BytesMut::new();
    write_varint_buf(25565, &mut buf);
    let frozen = buf.freeze();

    c.bench_function("varint_buf_read", |b| {
        b.iter(|| {
            let mut data = black_box(frozen.clone());
            black_box(read_varint_buf(&mut data)).ok();
        });
    });
}

fn bench_varint_size(c: &mut Criterion) {
    let values = [0, 1, 127, 128, 16383, 2_097_151, 268_435_455, i32::MAX, -1];

    c.bench_function("varint_size_9_values", |b| {
        b.iter(|| {
            for &v in &values {
                black_box(varint_size(v));
            }
        });
    });
}

// -----------------------------------------------------------------------
// Packet encode / decode (using a simple handshake-like payload)
// -----------------------------------------------------------------------

fn bench_packet_frame_encode(c: &mut Criterion) {
    // Simulate encoding a small packet: VarInt packet ID + body
    let body = b"localhost";
    let packet_id: i32 = 0x00;

    c.bench_function("packet_frame_encode", |b| {
        b.iter(|| {
            let mut inner = BytesMut::with_capacity(32);
            write_varint_buf(packet_id, &mut inner);
            inner.extend_from_slice(body);
            // Frame it: length prefix + inner
            let mut frame = BytesMut::with_capacity(inner.len() + 5);
            write_varint_buf(inner.len() as i32, &mut frame);
            frame.extend_from_slice(&inner);
            black_box(frame);
        });
    });
}

fn bench_packet_frame_decode(c: &mut Criterion) {
    let body = b"localhost";
    let packet_id: i32 = 0x00;

    let mut inner = BytesMut::with_capacity(32);
    write_varint_buf(packet_id, &mut inner);
    inner.extend_from_slice(body);
    let mut frame = BytesMut::with_capacity(inner.len() + 5);
    write_varint_buf(inner.len() as i32, &mut frame);
    frame.extend_from_slice(&inner);
    let frozen = frame.freeze();

    c.bench_function("packet_frame_decode", |b| {
        b.iter(|| {
            let mut data: Bytes = black_box(frozen.clone());
            // Read length prefix
            let _len = read_varint_buf(&mut data).ok();
            // Read packet ID
            let _id = read_varint_buf(&mut data).ok();
            // Remaining is body
            black_box(data);
        });
    });
}

criterion_group!(
    benches,
    bench_varint_encode_small,
    bench_varint_encode_large,
    bench_varint_decode_small,
    bench_varint_decode_large,
    bench_varint_roundtrip,
    bench_varlong_encode,
    bench_varlong_decode,
    bench_varint_buf_write,
    bench_varint_buf_read,
    bench_varint_size,
    bench_packet_frame_encode,
    bench_packet_frame_decode,
);
criterion_main!(benches);
