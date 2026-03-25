//! Fuzz target: feed arbitrary bytes into the NBT reader.
//!
//! This exercises `read_nbt` with a bounded accounter to catch panics,
//! hangs, or memory overuse on adversarial input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidized_nbt::{NbtAccounter, read_nbt};

fuzz_target!(|data: &[u8]| {
    // 2 MiB limit matches vanilla's network NBT size cap.
    let mut accounter = NbtAccounter::new(2 * 1024 * 1024);
    let mut cursor = std::io::Cursor::new(data);
    // We only care that this doesn't panic or hang — errors are fine.
    let _ = read_nbt(&mut cursor, &mut accounter);
});
