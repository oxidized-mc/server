//! Fuzz target: feed arbitrary bytes as a packet frame payload.
//!
//! Simulates reading a VarInt packet ID followed by body bytes,
//! verifying that the codec layer never panics on malformed data.

#![no_main]

use bytes::Bytes;
use libfuzzer_sys::fuzz_target;
use oxidized_protocol::codec::varint::read_varint_buf;

fuzz_target!(|data: &[u8]| {
    let mut buf = Bytes::copy_from_slice(data);

    // Try to read a packet ID from the fuzzed bytes.
    if let Ok(_packet_id) = read_varint_buf(&mut buf) {
        // The remaining bytes would be the packet body.
        // Just ensure nothing panics when we consume whatever is left.
        let _ = buf.len();
    }
});
