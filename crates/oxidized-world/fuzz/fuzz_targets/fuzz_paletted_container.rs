//! Fuzz target: feed arbitrary bytes into the paletted container decoder.
//!
//! Exercises `PalettedContainer::read_from_bytes` with both block-state
//! and biome strategies to catch panics on malformed chunk section data.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidized_world::chunk::paletted_container::{PalettedContainer, Strategy};

fuzz_target!(|data: &[u8]| {
    // Try decoding as block states (4096 entries, the common hot path).
    let mut cursor = data;
    let _ = PalettedContainer::read_from_bytes(Strategy::BlockStates, &mut cursor);

    // Try decoding as biomes (64 entries).
    let mut cursor = data;
    let _ = PalettedContainer::read_from_bytes(Strategy::Biomes, &mut cursor);
});
