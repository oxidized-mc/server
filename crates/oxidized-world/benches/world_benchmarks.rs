//! Benchmarks for oxidized-world: block state lookup, tag membership check.
#![allow(missing_docs, clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidized_registry::{BlockStateId, BlockTags};

// -----------------------------------------------------------------------
// Block state lookups
// -----------------------------------------------------------------------

fn bench_block_state_data(c: &mut Criterion) {
    // Stone is state 1, a very common lookup
    let stone = BlockStateId(1);

    c.bench_function("block_state_data_lookup", |b| {
        b.iter(|| {
            let entry = black_box(stone).data();
            black_box(entry);
        });
    });
}

fn bench_block_state_name(c: &mut Criterion) {
    let stone = BlockStateId(1);

    c.bench_function("block_state_name_lookup", |b| {
        b.iter(|| {
            let name = black_box(stone).block_name();
            black_box(name);
        });
    });
}

fn bench_block_state_flags(c: &mut Criterion) {
    let stone = BlockStateId(1);

    c.bench_function("block_state_flags_check", |b| {
        b.iter(|| {
            let id = black_box(stone);
            black_box(id.is_air());
            black_box(id.is_solid());
            black_box(id.has_collision());
            black_box(id.is_opaque());
        });
    });
}

fn bench_block_state_physics_properties(c: &mut Criterion) {
    // Ice (slippery block) — good for testing friction/speed/jump lookups
    // Ice default state is around 5765, but let's use a range scan approach:
    // We just pick a representative state ID
    let state = BlockStateId(1); // stone

    c.bench_function("block_state_physics_properties", |b| {
        b.iter(|| {
            let id = black_box(state);
            black_box(id.friction());
            black_box(id.speed_factor());
            black_box(id.jump_factor());
            black_box(id.hardness());
            black_box(id.explosion_resistance());
        });
    });
}

fn bench_block_state_is_air_batch(c: &mut Criterion) {
    // Check is_air for 1000 sequential states (simulates chunk iteration)
    c.bench_function("block_state_is_air_1000_states", |b| {
        b.iter(|| {
            for i in 0..1000u16 {
                black_box(BlockStateId(i).is_air());
            }
        });
    });
}

fn bench_block_state_with_property(c: &mut Criterion) {
    // Oak stairs have multiple properties — good for with_property benchmark
    // Use state 0 as starting point to find one with properties
    let stone = BlockStateId(1);

    c.bench_function("block_state_with_property", |b| {
        b.iter(|| {
            // Attempt a property change (may return None for blocks without that property)
            black_box(black_box(stone).with_property("facing", "north"));
        });
    });
}

// -----------------------------------------------------------------------
// Tag membership checks
// -----------------------------------------------------------------------

fn bench_tag_contains_hit(c: &mut Criterion) {
    let tags = BlockTags;
    // Block type 1 is stone — check if it's in "minecraft:mineable/pickaxe"
    c.bench_function("tag_contains_hit", |b| {
        b.iter(|| {
            black_box(tags.contains(black_box("minecraft:mineable/pickaxe"), black_box(1)));
        });
    });
}

fn bench_tag_contains_miss(c: &mut Criterion) {
    let tags = BlockTags;
    // Block type 1 (stone) is NOT in "minecraft:doors"
    c.bench_function("tag_contains_miss", |b| {
        b.iter(|| {
            black_box(tags.contains(black_box("minecraft:doors"), black_box(1)));
        });
    });
}

fn bench_tag_get(c: &mut Criterion) {
    let tags = BlockTags;

    c.bench_function("tag_get_lookup", |b| {
        b.iter(|| {
            let set = tags.get(black_box("minecraft:mineable/pickaxe"));
            black_box(set);
        });
    });
}

fn bench_tag_membership_batch(c: &mut Criterion) {
    let tags = BlockTags;

    c.bench_function("tag_membership_100_checks", |b| {
        b.iter(|| {
            for id in 0..100u16 {
                black_box(tags.contains("minecraft:mineable/pickaxe", id));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_block_state_data,
    bench_block_state_name,
    bench_block_state_flags,
    bench_block_state_physics_properties,
    bench_block_state_is_air_batch,
    bench_block_state_with_property,
    bench_tag_contains_hit,
    bench_tag_contains_miss,
    bench_tag_get,
    bench_tag_membership_batch,
);
criterion_main!(benches);
