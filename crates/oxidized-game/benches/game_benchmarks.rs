//! Benchmarks for oxidized-game: block property access via BlockStateId.
#![allow(missing_docs, clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use oxidized_world::registry::BlockStateId;

// -----------------------------------------------------------------------
// Block property access (game-layer perspective)
// -----------------------------------------------------------------------

fn bench_friction_lookup(c: &mut Criterion) {
    // Various block states to test friction lookup (hot path in physics tick)
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("friction_lookup_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                black_box(state.friction());
            }
        });
    });
}

fn bench_speed_factor_lookup(c: &mut Criterion) {
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("speed_factor_lookup_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                black_box(state.speed_factor());
            }
        });
    });
}

fn bench_jump_factor_lookup(c: &mut Criterion) {
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("jump_factor_lookup_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                black_box(state.jump_factor());
            }
        });
    });
}

fn bench_combined_physics_properties(c: &mut Criterion) {
    // Simulate the typical physics tick access pattern: friction + speed + jump
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("combined_physics_props_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                let s = black_box(state);
                black_box(s.friction());
                black_box(s.speed_factor());
                black_box(s.jump_factor());
            }
        });
    });
}

fn bench_light_properties(c: &mut Criterion) {
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("light_properties_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                let s = black_box(state);
                black_box(s.light_emission());
                black_box(s.light_opacity());
            }
        });
    });
}

fn bench_block_categorization(c: &mut Criterion) {
    // Simulate checking block category (common in game logic)
    let states: Vec<BlockStateId> = (0..100).map(BlockStateId).collect();

    c.bench_function("block_categorization_100_states", |b| {
        b.iter(|| {
            for &state in &states {
                let s = black_box(state);
                black_box(s.is_air());
                black_box(s.is_liquid());
                black_box(s.is_solid());
                black_box(s.is_replaceable());
            }
        });
    });
}

criterion_group!(
    benches,
    bench_friction_lookup,
    bench_speed_factor_lookup,
    bench_jump_factor_lookup,
    bench_combined_physics_properties,
    bench_light_properties,
    bench_block_categorization,
);
criterion_main!(benches);
