//! Benchmarks for oxidized-nbt: NBT parse/write roundtrip, SNBT formatting,
//! compound lookups.
#![allow(missing_docs, clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidized_nbt::{
    NbtAccounter, NbtCompound, NbtList, NbtTag, TAG_COMPOUND, format_snbt, format_snbt_pretty,
    parse_snbt, read_nbt, write_nbt,
};

/// Builds a representative compound tag resembling a chunk's block entity data.
fn sample_compound() -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_string("id", "minecraft:chest");
    root.put_int("x", 128);
    root.put_int("y", 64);
    root.put_int("z", -256);
    root.put_byte("keepPacked", 0);

    let mut items = NbtList::new(TAG_COMPOUND);
    for i in 0..27 {
        let mut item = NbtCompound::new();
        item.put_string("id", "minecraft:diamond");
        item.put_byte("count", 64);
        item.put_byte("slot", i);
        items.push(NbtTag::Compound(item)).unwrap();
    }
    root.put("Items", NbtTag::List(items));
    root
}

/// Builds a larger compound tag for heavier benchmarks.
fn large_compound() -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_int("DataVersion", 4786);

    let mut level = NbtCompound::new();
    level.put_int("xPos", 0);
    level.put_int("zPos", 0);
    level.put_string("Status", "minecraft:full");
    level.put_long("LastUpdate", 123_456_789);
    level.put_long("InhabitedTime", 42_000);

    // Simulate section palette entries
    let mut sections = NbtList::new(TAG_COMPOUND);
    for y in -4..20 {
        let mut section = NbtCompound::new();
        section.put_byte("Y", y);

        let mut palette = NbtList::new(TAG_COMPOUND);
        for _ in 0..16 {
            let mut entry = NbtCompound::new();
            entry.put_string("Name", "minecraft:stone");
            palette.push(NbtTag::Compound(entry)).unwrap();
        }
        section.put("block_states", NbtTag::List(palette));
        section.put(
            "data",
            NbtTag::LongArray(vec![0x0123_4567_89AB_CDEFi64; 256]),
        );
        sections.push(NbtTag::Compound(section)).unwrap();
    }
    level.put("sections", NbtTag::List(sections));
    root.put("Level", NbtTag::Compound(level));
    root
}

fn bench_nbt_write_small(c: &mut Criterion) {
    let compound = sample_compound();
    c.bench_function("nbt_write_small", |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(1024);
            write_nbt(&mut buf, black_box(&compound)).ok();
            black_box(buf);
        });
    });
}

fn bench_nbt_read_small(c: &mut Criterion) {
    let compound = sample_compound();
    let mut buf = Vec::new();
    write_nbt(&mut buf, &compound).ok();
    let bytes = buf;

    c.bench_function("nbt_read_small", |b| {
        b.iter(|| {
            let mut cursor = std::io::Cursor::new(black_box(&bytes));
            let mut acc = NbtAccounter::new(usize::MAX);
            let result = read_nbt(&mut cursor, &mut acc);
            black_box(result).ok();
        });
    });
}

fn bench_nbt_roundtrip_small(c: &mut Criterion) {
    let compound = sample_compound();
    let mut encoded = Vec::new();
    write_nbt(&mut encoded, &compound).ok();

    c.bench_function("nbt_roundtrip_small", |b| {
        b.iter(|| {
            // Write
            let mut buf = Vec::with_capacity(encoded.len());
            write_nbt(&mut buf, black_box(&compound)).ok();
            // Read back
            let mut cursor = std::io::Cursor::new(&buf);
            let mut acc = NbtAccounter::new(usize::MAX);
            black_box(read_nbt(&mut cursor, &mut acc)).ok();
        });
    });
}

fn bench_nbt_write_large(c: &mut Criterion) {
    let compound = large_compound();
    c.bench_function("nbt_write_large", |b| {
        b.iter(|| {
            let mut buf = Vec::with_capacity(64 * 1024);
            write_nbt(&mut buf, black_box(&compound)).ok();
            black_box(buf);
        });
    });
}

fn bench_nbt_read_large(c: &mut Criterion) {
    let compound = large_compound();
    let mut buf = Vec::new();
    write_nbt(&mut buf, &compound).ok();
    let bytes = buf;

    c.bench_function("nbt_read_large", |b| {
        b.iter(|| {
            let mut cursor = std::io::Cursor::new(black_box(&bytes));
            let mut acc = NbtAccounter::new(usize::MAX);
            let result = read_nbt(&mut cursor, &mut acc);
            black_box(result).ok();
        });
    });
}

fn bench_snbt_format(c: &mut Criterion) {
    let compound = sample_compound();
    let tag = NbtTag::Compound(compound);

    c.bench_function("snbt_format", |b| {
        b.iter(|| {
            let s = format_snbt(black_box(&tag));
            black_box(s);
        });
    });
}

fn bench_snbt_format_pretty(c: &mut Criterion) {
    let compound = sample_compound();
    let tag = NbtTag::Compound(compound);

    c.bench_function("snbt_format_pretty", |b| {
        b.iter(|| {
            let s = format_snbt_pretty(black_box(&tag), 2);
            black_box(s);
        });
    });
}

fn bench_snbt_parse(c: &mut Criterion) {
    let compound = sample_compound();
    let tag = NbtTag::Compound(compound);
    let snbt_str = format_snbt(&tag);

    c.bench_function("snbt_parse", |b| {
        b.iter(|| {
            let result = parse_snbt(black_box(&snbt_str));
            black_box(result).ok();
        });
    });
}

fn bench_compound_lookup(c: &mut Criterion) {
    let compound = sample_compound();

    c.bench_function("compound_get_string", |b| {
        b.iter(|| {
            black_box(compound.get_string(black_box("id")));
        });
    });
}

fn bench_compound_insert(c: &mut Criterion) {
    c.bench_function("compound_insert_50_keys", |b| {
        b.iter(|| {
            let mut compound = NbtCompound::new();
            for i in 0..50 {
                compound.put_int(format!("key_{i}"), i);
            }
            black_box(compound);
        });
    });
}

criterion_group!(
    benches,
    bench_nbt_write_small,
    bench_nbt_read_small,
    bench_nbt_roundtrip_small,
    bench_nbt_write_large,
    bench_nbt_read_large,
    bench_snbt_format,
    bench_snbt_format_pretty,
    bench_snbt_parse,
    bench_compound_lookup,
    bench_compound_insert,
);
criterion_main!(benches);
