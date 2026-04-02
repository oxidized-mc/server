//! Fuzz target: feed arbitrary bytes into VarInt and VarLong decoders.
//!
//! Ensures `decode_varint` and `decode_varlong` never panic on any input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidized_codec::varint::{decode_varint, decode_varlong};

fuzz_target!(|data: &[u8]| {
    let _ = decode_varint(data);
    let _ = decode_varlong(data);
});
