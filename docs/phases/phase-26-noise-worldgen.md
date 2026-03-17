# Phase 26 — Noise-Based World Generation

**Crate:** `oxidized-game`  
**Reward:** New worlds generate realistic overworld terrain with biomes.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-001: Async Runtime](../adr/adr-001-async-runtime.md) — Tokio runtime selection and async patterns
- [ADR-016: Worldgen Pipeline](../adr/adr-016-worldgen-pipeline.md) — Rayon thread pool with dependency-aware scheduling


## Goal

Implement the full vanilla overworld generation pipeline:
`NoiseBasedChunkGenerator` driving seven sequential steps (noise sampling,
density-function evaluation, aquifer placement, surface rules, carvers, and
feature decoration), backed by `MultiNoiseBiomeSource` for biome selection and
a suite of noise primitives (`PerlinNoise`, `SimplexNoise`, `NormalNoise`).
Chunk generation must run off the main thread (tokio blocking pool) and produce
terrain indistinguishable from vanilla.

---

## Java Reference

| Concept | Java class | Path |
|---------|-----------|------|
| Chunk generator | `NoiseBasedChunkGenerator` | `net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator` |
| Density function registry | `DensityFunctions` | `net.minecraft.world.level.levelgen.DensityFunctions` |
| Noise chunk | `NoiseChunk` | `net.minecraft.world.level.levelgen.NoiseChunk` |
| Aquifer | `Aquifer` | `net.minecraft.world.level.levelgen.Aquifer` |
| Surface rules | `SurfaceRules` | `net.minecraft.world.level.levelgen.SurfaceRules` |
| Surface rule data | `SurfaceRuleData` | `net.minecraft.data.worldgen.SurfaceRuleData` |
| Cave carver | `CaveCarver` | `net.minecraft.world.level.levelgen.carver.CaveCarver` |
| Canyon carver | `CanyonCarver` | `net.minecraft.world.level.levelgen.carver.CanyonCarver` |
| Biome source | `MultiNoiseBiomeSource` | `net.minecraft.world.level.biome.MultiNoiseBiomeSource` |
| Normal noise | `NormalNoise` | `net.minecraft.world.level.levelgen.synth.NormalNoise` |
| Perlin noise | `PerlinNoise` | `net.minecraft.world.level.levelgen.synth.PerlinNoise` |
| Simplex noise | `SimplexNoise` | `net.minecraft.world.level.levelgen.synth.SimplexNoise` |
| Worldgen random | `WorldgenRandom` | `net.minecraft.world.level.levelgen.WorldgenRandom` |
| XoroshiroRandom | `XoroshiroRandomSource` | `net.minecraft.world.level.levelgen.XoroshiroRandomSource` |

---

## Tasks

### 26.1 — Noise primitives: `PerlinNoise`, `SimplexNoise`, `NormalNoise`

```rust
// crates/oxidized-game/src/worldgen/noise/perlin.rs

/// Single octave of gradient noise. Matches `ImprovedNoise` in Java.
pub struct ImprovedNoise {
    permutation: [u8; 256],
    pub x_offset: f64,
    pub y_offset: f64,
    pub z_offset: f64,
}

impl ImprovedNoise {
    pub fn new(rng: &mut impl RngSource) -> Self {
        let xo = rng.next_f64() * 256.0;
        let yo = rng.next_f64() * 256.0;
        let zo = rng.next_f64() * 256.0;
        let mut perm = [0u8; 256];
        for (i, p) in perm.iter_mut().enumerate() { *p = i as u8; }
        for i in (1..256).rev() {
            let j = rng.next_int_bounded((i + 1) as i32) as usize;
            perm.swap(i, j);
        }
        Self { permutation: perm, x_offset: xo, y_offset: yo, z_offset: zo }
    }

    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let x = x + self.x_offset;
        let y = y + self.y_offset;
        let z = z + self.z_offset;
        let xi = x.floor() as i32 & 0xFF;
        let yi = y.floor() as i32 & 0xFF;
        let zi = z.floor() as i32 & 0xFF;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let zf = z - z.floor();
        let u = Self::fade(xf);
        let v = Self::fade(yf);
        let w = Self::fade(zf);
        let p = &self.permutation;
        let a  = p[xi as usize]       as usize + yi as usize;
        let aa = p[a  & 0xFF]         as usize + zi as usize;
        let ab = p[(a + 1) & 0xFF]    as usize + zi as usize;
        let b  = p[(xi + 1) as usize & 0xFF] as usize + yi as usize;
        let ba = p[b  & 0xFF]         as usize + zi as usize;
        let bb = p[(b + 1) & 0xFF]    as usize + zi as usize;
        Self::lerp3(u, v, w,
            Self::grad(p[aa & 0xFF], xf, yf, zf),
            Self::grad(p[ba & 0xFF], xf - 1.0, yf, zf),
            Self::grad(p[ab & 0xFF], xf, yf - 1.0, zf),
            Self::grad(p[bb & 0xFF], xf - 1.0, yf - 1.0, zf),
            Self::grad(p[(aa + 1) & 0xFF], xf, yf, zf - 1.0),
            Self::grad(p[(ba + 1) & 0xFF], xf - 1.0, yf, zf - 1.0),
            Self::grad(p[(ab + 1) & 0xFF], xf, yf - 1.0, zf - 1.0),
            Self::grad(p[(bb + 1) & 0xFF], xf - 1.0, yf - 1.0, zf - 1.0),
        )
    }

    fn fade(t: f64) -> f64 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }

    fn lerp(t: f64, a: f64, b: f64) -> f64 { a + t * (b - a) }

    fn lerp3(u: f64, v: f64, w: f64,
             a: f64, b: f64, c: f64, d: f64,
             e: f64, f: f64, g: f64, h: f64) -> f64 {
        Self::lerp(w,
            Self::lerp(v, Self::lerp(u, a, b), Self::lerp(u, c, d)),
            Self::lerp(v, Self::lerp(u, e, f), Self::lerp(u, g, h)),
        )
    }

    fn grad(hash: u8, x: f64, y: f64, z: f64) -> f64 {
        match hash & 0xF {
            0x0 =>  x + y, 0x1 => -x + y, 0x2 =>  x - y, 0x3 => -x - y,
            0x4 =>  x + z, 0x5 => -x + z, 0x6 =>  x - z, 0x7 => -x - z,
            0x8 =>  y + z, 0x9 => -y + z, 0xA =>  y - z, 0xB => -y - z,
            0xC =>  y + x, 0xD => -y + z, 0xE =>  y - x, 0xF => -y - z,
            _ => unreachable!(),
        }
    }
}

/// Fractal Brownian Motion noise: multiple `ImprovedNoise` octaves summed.
/// `lacunarity = 2.0`, `persistence = 0.5`. Matches `PerlinNoise` in Java.
pub struct PerlinNoise {
    octaves: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    lowest_freq_input_factor: f64,
    lowest_freq_value_factor: f64,
}

impl PerlinNoise {
    /// Create from an ordered list of `(octave_index, amplitude)` pairs.
    /// `octave_index` is relative to the highest octave (index 0 = highest frequency).
    pub fn new(rng: &mut impl RngSource, octaves: &[(i32, f64)]) -> Self {
        // Java: PerlinNoise.create(RandomSource, IntStream, DoubleList)
        // Build octave list, skip those with amplitude == 0.0
        todo!()
    }

    /// Sample the sum of all octaves at (x, y, z).
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut freq = self.lowest_freq_input_factor;
        let mut amp  = self.lowest_freq_value_factor;
        for octave in &self.octaves {
            if let Some(noise) = octave {
                value += noise.noise(x * freq, y * freq, z * freq) * amp;
            }
            freq *= 2.0;
            amp  *= 0.5;
        }
        value
    }
}

// crates/oxidized-game/src/worldgen/noise/normal_noise.rs

/// Two `PerlinNoise` samplers combined: `(A(x,y,z) + B(x+a,y+b,z+c)) * factor`.
/// Matches `NormalNoise` in Java (used for biome climate parameters).
pub struct NormalNoise {
    value_factor: f64,
    first: PerlinNoise,
    second: PerlinNoise,
}

impl NormalNoise {
    pub fn new(rng: &mut impl RngSource, amplitudes: &[f64]) -> Self {
        let first  = PerlinNoise::new(rng, /* build octave list from amplitudes */ &[]);
        let second = PerlinNoise::new(rng, &[]);
        let max_value = Self::max_value(amplitudes);
        Self {
            value_factor: 1.0 / (max_value * 2.0 / 3.0),
            first,
            second,
        }
    }

    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * 1.0181268882175227;
        let y2 = y * 1.0181268882175227;
        let z2 = z * 1.0181268882175227;
        (self.first.sample(x, y, z) + self.second.sample(x2, y2, z2)) * self.value_factor
    }

    fn max_value(amplitudes: &[f64]) -> f64 {
        let max_amp: f64 = amplitudes.iter().cloned().fold(0.0_f64, f64::max);
        let octave_count = amplitudes.iter().filter(|&&a| a != 0.0).count() as f64;
        1.0 / (2.0 - 2.0_f64.powf(1.0 - octave_count)) * max_amp
    }
}

// crates/oxidized-game/src/worldgen/noise/simplex.rs

/// 3D simplex noise. Used for terrain shift/offset functions.
/// Matches `SimplexNoise` in Java.
pub struct SimplexNoise {
    perm: [i32; 512],
    pub x_offset: f64,
    pub y_offset: f64,
    pub z_offset: f64,
}

const GRAD3: [[i32; 3]; 16] = [
    [1,1,0],[-1,1,0],[1,-1,0],[-1,-1,0],
    [1,0,1],[-1,0,1],[1,0,-1],[-1,0,-1],
    [0,1,1],[0,-1,1],[0,1,-1],[0,-1,-1],
    [1,1,0],[0,-1,1],[-1,1,0],[0,-1,-1],
];

impl SimplexNoise {
    pub fn new(rng: &mut impl RngSource) -> Self {
        let xo = rng.next_f64() * 256.0;
        let yo = rng.next_f64() * 256.0;
        let zo = rng.next_f64() * 256.0;
        let mut p = [0i32; 256];
        for (i, v) in p.iter_mut().enumerate() { *v = i as i32; }
        for i in (1..256).rev() {
            let j = rng.next_int_bounded((i + 1) as i32) as usize;
            p.swap(i, j);
        }
        let mut perm = [0i32; 512];
        for i in 0..512 { perm[i] = p[i & 255]; }
        Self { perm, x_offset: xo, y_offset: yo, z_offset: zo }
    }

    pub fn getValue(&self, x: f64, y: f64) -> f64 {
        const F2: f64 = 0.3660254037844386;
        const G2: f64 = 0.21132486540518713;
        let s = (x + y) * F2;
        let i = (x + s).floor() as i32;
        let j = (y + s).floor() as i32;
        let t = (i + j) as f64 * G2;
        let x0 = x - (i as f64 - t);
        let y0 = y - (j as f64 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;
        let gi0 = self.perm[ii + self.perm[jj] as usize] as usize % 12;
        let gi1 = self.perm[ii + i1 + self.perm[jj + j1] as usize] as usize % 12;
        let gi2 = self.perm[ii + 1 + self.perm[jj + 1] as usize] as usize % 12;
        let n0 = Self::corner(gi0, x0, y0, 0.0);
        let n1 = Self::corner(gi1, x1, y1, 0.0);
        let n2 = Self::corner(gi2, x2, y2, 0.0);
        70.0 * (n0 + n1 + n2)
    }

    fn corner(gi: usize, x: f64, y: f64, z: f64) -> f64 {
        let t = 0.5 - x * x - y * y - z * z;
        if t < 0.0 {
            0.0
        } else {
            let t2 = t * t;
            let g = GRAD3[gi];
            t2 * t2 * (g[0] as f64 * x + g[1] as f64 * y + g[2] as f64 * z)
        }
    }
}
```

### 26.2 — `WorldgenRandom` and `XoroshiroRandomSource`

```rust
// crates/oxidized-game/src/worldgen/random.rs

/// Random source backed by XoroshiroRandomSource (128-bit state).
/// Matches Java's `net.minecraft.world.level.levelgen.XoroshiroRandomSource`.
pub struct XoroshiroRandomSource {
    lo: u64,
    hi: u64,
}

impl XoroshiroRandomSource {
    pub fn new(seed_lo: u64, seed_hi: u64) -> Self {
        let (lo, hi) = Self::mix_stafford_13(seed_lo, seed_hi);
        Self { lo, hi }
    }

    /// Seed from a world seed + position key. Mirrors Java's `RandomSupport.seedUniquifier`.
    pub fn from_seed(world_seed: i64) -> Self {
        let s = world_seed as u64;
        Self::new(s ^ 0x6A09E667F3BCC908, s ^ 0xBB67AE8584CAA73B)
    }

    pub fn next_long(&mut self) -> u64 {
        let lo = self.lo;
        let hi = self.hi;
        let result = lo.wrapping_add(hi).rotate_left(17).wrapping_add(lo);
        let new_hi = lo ^ hi;
        self.lo = lo.rotate_left(49) ^ new_hi ^ (new_hi << 21);
        self.hi = new_hi.rotate_left(28);
        result
    }

    pub fn next_int(&mut self) -> i32 {
        (self.next_long() >> 32) as i32
    }

    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        let bound = bound as u64;
        let mut bits;
        let mut val;
        loop {
            bits = (self.next_long() >> 33) & 0x7FFF_FFFF;
            val  = bits % bound;
            if bits - val + (bound - 1) < u64::MAX / 2 { break; }
        }
        val as i32
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_long() >> 11) as f64 * 1.1102230246251565e-16
    }

    fn mix_stafford_13(mut z: u64, mut w: u64) -> (u64, u64) {
        z = z.wrapping_add(0x9E3779B97F4A7C15);
        w = w.wrapping_add(0x6C62272E07BB0142);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        w = (w ^ (w >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        w = (w ^ (w >> 27)).wrapping_mul(0x94D049BB133111EB);
        w ^= w >> 31;
        (z, w)
    }
}

pub trait RngSource: Send {
    fn next_long(&mut self) -> u64;
    fn next_int(&mut self) -> i32;
    fn next_int_bounded(&mut self, bound: i32) -> i32;
    fn next_f64(&mut self) -> f64;
    fn fork(&mut self) -> Box<dyn RngSource>;
    fn fork_positional(&mut self) -> Box<dyn PositionalRandomFactory>;
}

pub trait PositionalRandomFactory: Send + Sync {
    fn at(&self, x: i32, y: i32, z: i32) -> Box<dyn RngSource>;
    fn from_hash_of(&self, name: &str) -> Box<dyn RngSource>;
}
```

### 26.3 — `DensityFunction` tree

```rust
// crates/oxidized-game/src/worldgen/density_function.rs

use std::sync::Arc;

/// Evaluation context passed to every density function node.
#[derive(Clone, Copy, Debug)]
pub struct FunctionContext {
    pub block_x: i32,
    pub block_y: i32,
    pub block_z: i32,
}

/// Core trait for composable density functions.
/// Mirrors `DensityFunction` in Java.
pub trait DensityFunction: Send + Sync {
    fn compute(&self, ctx: FunctionContext) -> f64;
    fn min_value(&self) -> f64;
    fn max_value(&self) -> f64;
}

pub type DfRef = Arc<dyn DensityFunction>;

// -- Primitive nodes --

pub struct ConstantDf(pub f64);
impl DensityFunction for ConstantDf {
    fn compute(&self, _: FunctionContext) -> f64 { self.0 }
    fn min_value(&self) -> f64 { self.0 }
    fn max_value(&self) -> f64 { self.0 }
}

pub struct NoiseDf {
    pub noise: Arc<NormalNoise>,
    pub xz_scale: f64,
    pub y_scale: f64,
}
impl DensityFunction for NoiseDf {
    fn compute(&self, ctx: FunctionContext) -> f64 {
        self.noise.sample(
            ctx.block_x as f64 * self.xz_scale,
            ctx.block_y as f64 * self.y_scale,
            ctx.block_z as f64 * self.xz_scale,
        )
    }
    fn min_value(&self) -> f64 { -1.0 }
    fn max_value(&self) -> f64 {  1.0 }
}

// -- Binary nodes --

pub struct AddDf(pub DfRef, pub DfRef);
impl DensityFunction for AddDf {
    fn compute(&self, ctx: FunctionContext) -> f64 { self.0.compute(ctx) + self.1.compute(ctx) }
    fn min_value(&self) -> f64 { self.0.min_value() + self.1.min_value() }
    fn max_value(&self) -> f64 { self.0.max_value() + self.1.max_value() }
}

pub struct MulDf(pub DfRef, pub DfRef);
impl DensityFunction for MulDf {
    fn compute(&self, ctx: FunctionContext) -> f64 { self.0.compute(ctx) * self.1.compute(ctx) }
    fn min_value(&self) -> f64 {
        let (a0, a1, b0, b1) = (self.0.min_value(), self.0.max_value(), self.1.min_value(), self.1.max_value());
        [a0*b0, a0*b1, a1*b0, a1*b1].iter().cloned().fold(f64::INFINITY, f64::min)
    }
    fn max_value(&self) -> f64 {
        let (a0, a1, b0, b1) = (self.0.min_value(), self.0.max_value(), self.1.min_value(), self.1.max_value());
        [a0*b0, a0*b1, a1*b0, a1*b1].iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }
}

pub struct ClampDf { pub input: DfRef, pub min: f64, pub max: f64 }
impl DensityFunction for ClampDf {
    fn compute(&self, ctx: FunctionContext) -> f64 {
        self.input.compute(ctx).clamp(self.min, self.max)
    }
    fn min_value(&self) -> f64 { self.min }
    fn max_value(&self) -> f64 { self.max }
}

/// Gradient along the Y axis clamped to a range.
pub struct YClampedGradientDf { pub from_y: i32, pub to_y: i32, pub from_v: f64, pub to_v: f64 }
impl DensityFunction for YClampedGradientDf {
    fn compute(&self, ctx: FunctionContext) -> f64 {
        let t = (ctx.block_y - self.from_y) as f64 / (self.to_y - self.from_y) as f64;
        self.from_v + (self.to_v - self.from_v) * t.clamp(0.0, 1.0)
    }
    fn min_value(&self) -> f64 { self.from_v.min(self.to_v) }
    fn max_value(&self) -> f64 { self.from_v.max(self.to_v) }
}

/// Caches computation for a flat (XZ-only) 2D slice, reused across all Y.
pub struct FlatCacheDf { pub inner: DfRef }
// (runtime caches last (x,z) result)
impl DensityFunction for FlatCacheDf {
    fn compute(&self, ctx: FunctionContext) -> f64 { self.inner.compute(ctx) }
    fn min_value(&self) -> f64 { self.inner.min_value() }
    fn max_value(&self) -> f64 { self.inner.max_value() }
}

/// Linearly interpolated version of the function — evaluated on the 4×8×4 coarse grid.
pub struct InterpolatedDf { pub inner: DfRef }
impl DensityFunction for InterpolatedDf {
    fn compute(&self, ctx: FunctionContext) -> f64 { self.inner.compute(ctx) }
    fn min_value(&self) -> f64 { self.inner.min_value() }
    fn max_value(&self) -> f64 { self.inner.max_value() }
}
```

### 26.4 — `NoiseChunk` (trilinear interpolation)

```rust
// crates/oxidized-game/src/worldgen/noise_chunk.rs

/// Evaluates a density-function tree on a coarse 4×8×4 grid over a 16×384×16 chunk
/// and trilinearly interpolates to produce per-block densities.
///
/// Coarse grid resolution:
///   x: 4 cells of 4 blocks each  (cell_width  = 4)
///   y: 8 cells of 8 blocks each  (cell_height = 8)  — covers 64 Y blocks per section
///   z: 4 cells of 4 blocks each
pub struct NoiseChunk {
    pub cell_count_x: usize, // 4
    pub cell_count_y: usize, // 48 (for -64..320)
    pub cell_count_z: usize, // 4
    pub cell_width: usize,   // 4 blocks
    pub cell_height: usize,  // 8 blocks
    /// Flat array [cell_x][cell_y][cell_z] of coarse density values.
    coarse: Vec<f64>,
    pub final_density: Arc<dyn DensityFunction>,
}

impl NoiseChunk {
    pub const CELL_WIDTH: usize  = 4;
    pub const CELL_HEIGHT: usize = 8;

    pub fn new(
        chunk_x: i32,
        chunk_z: i32,
        min_y: i32,
        height: i32,
        final_density: Arc<dyn DensityFunction>,
    ) -> Self {
        let cell_count_x = 16 / Self::CELL_WIDTH;
        let cell_count_z = 16 / Self::CELL_WIDTH;
        let cell_count_y = height as usize / Self::CELL_HEIGHT;
        let size = (cell_count_x + 1) * (cell_count_y + 1) * (cell_count_z + 1);
        let mut coarse = vec![0.0f64; size];

        // Pre-sample density function at every coarse grid corner.
        for cx in 0..=cell_count_x {
            for cy in 0..=cell_count_y {
                for cz in 0..=cell_count_z {
                    let bx = chunk_x * 16 + cx as i32 * Self::CELL_WIDTH as i32;
                    let by = min_y    + cy as i32 * Self::CELL_HEIGHT as i32;
                    let bz = chunk_z * 16 + cz as i32 * Self::CELL_WIDTH as i32;
                    let idx = cx * (cell_count_y + 1) * (cell_count_z + 1)
                              + cy * (cell_count_z + 1)
                              + cz;
                    coarse[idx] = final_density.compute(FunctionContext { block_x: bx, block_y: by, block_z: bz });
                }
            }
        }

        Self { cell_count_x, cell_count_y, cell_count_z,
               cell_width: Self::CELL_WIDTH, cell_height: Self::CELL_HEIGHT,
               coarse, final_density }
    }

    /// Get the trilinearly interpolated density at block-local coordinates (lx, ly, lz).
    /// `lx` in 0..16, `ly` in 0..height, `lz` in 0..16.
    pub fn density_at_block(&self, lx: usize, ly: usize, lz: usize) -> f64 {
        let cx = lx / self.cell_width;
        let cz = lz / self.cell_width;
        let cy = ly / self.cell_height;
        let tx = (lx % self.cell_width)  as f64 / self.cell_width  as f64;
        let ty = (ly % self.cell_height) as f64 / self.cell_height as f64;
        let tz = (lz % self.cell_width)  as f64 / self.cell_width  as f64;
        let stride_z = self.cell_count_z + 1;
        let stride_y = stride_z;
        let stride_x = (self.cell_count_y + 1) * stride_z;

        let c000 = self.coarse[cx * stride_x + cy * stride_y + cz];
        let c001 = self.coarse[cx * stride_x + cy * stride_y + cz + 1];
        let c010 = self.coarse[cx * stride_x + (cy+1) * stride_y + cz];
        let c011 = self.coarse[cx * stride_x + (cy+1) * stride_y + cz + 1];
        let c100 = self.coarse[(cx+1) * stride_x + cy * stride_y + cz];
        let c101 = self.coarse[(cx+1) * stride_x + cy * stride_y + cz + 1];
        let c110 = self.coarse[(cx+1) * stride_x + (cy+1) * stride_y + cz];
        let c111 = self.coarse[(cx+1) * stride_x + (cy+1) * stride_y + cz + 1];

        Self::trilinear(tx, ty, tz, c000, c001, c010, c011, c100, c101, c110, c111)
    }

    fn trilinear(tx: f64, ty: f64, tz: f64,
                 c000: f64, c001: f64, c010: f64, c011: f64,
                 c100: f64, c101: f64, c110: f64, c111: f64) -> f64 {
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        lerp(tx,
            lerp(ty, lerp(tz, c000, c001), lerp(tz, c010, c011)),
            lerp(ty, lerp(tz, c100, c101), lerp(tz, c110, c111)),
        )
    }
}
```

### 26.5 — `MultiNoiseBiomeSource`

```rust
// crates/oxidized-game/src/worldgen/biome_source.rs

/// Biome climate parameters (6-dimensional point in climate space).
/// All values are quantized to i64 in the range [-10000, 10000].
#[derive(Debug, Clone, Copy)]
pub struct Climate {
    pub temperature:     f32,
    pub humidity:        f32,
    pub continentalness: f32,
    pub erosion:         f32,
    pub depth:           f32,
    pub weirdness:       f32,
}

/// An entry mapping a climate parameter range to a biome.
#[derive(Clone)]
pub struct BiomePoint {
    pub temperature:     ParameterRange,
    pub humidity:        ParameterRange,
    pub continentalness: ParameterRange,
    pub erosion:         ParameterRange,
    pub depth:           ParameterRange,
    pub weirdness:       ParameterRange,
    pub offset:          f32,
    pub biome:           ResourceLocation,
}

/// [min, max] range for a single climate axis.
#[derive(Clone, Copy, Debug)]
pub struct ParameterRange {
    pub min: f32,
    pub max: f32,
}

impl ParameterRange {
    pub fn point(value: f32) -> Self { Self { min: value, max: value } }
    pub fn span(min: f32, max: f32) -> Self { Self { min, max } }

    /// Squared distance from a point to this range.
    pub fn distance(&self, value: f32) -> f32 {
        let d = if value < self.min { self.min - value }
                else if value > self.max { value - self.max }
                else { 0.0 };
        d * d
    }
}

/// Biome source using 6D noise-space nearest-point lookup.
/// Matches `MultiNoiseBiomeSource` in Java.
pub struct MultiNoiseBiomeSource {
    pub parameters: Vec<BiomePoint>,
    pub temperature_noise:     NormalNoise,
    pub humidity_noise:        NormalNoise,
    pub continentalness_noise: NormalNoise,
    pub erosion_noise:         NormalNoise,
    pub weirdness_noise:       NormalNoise,
    pub depth_noise:           NormalNoise,
}

impl MultiNoiseBiomeSource {
    /// Get the biome at block coordinates (x, y, z).
    pub fn get_biome(&self, x: i32, y: i32, z: i32) -> &ResourceLocation {
        let qx = x as f64 / 4.0;
        let qy = y as f64 / 4.0;
        let qz = z as f64 / 4.0;
        let climate = Climate {
            temperature:     self.temperature_noise.sample(qx, qy, qz) as f32,
            humidity:        self.humidity_noise.sample(qx, qy, qz) as f32,
            continentalness: self.continentalness_noise.sample(qx, 0.0, qz) as f32,
            erosion:         self.erosion_noise.sample(qx, 0.0, qz) as f32,
            weirdness:       self.weirdness_noise.sample(qx, 0.0, qz) as f32,
            depth:           0.0, // set from density function
        };
        self.find_closest_biome(&climate)
    }

    fn find_closest_biome(&self, c: &Climate) -> &ResourceLocation {
        self.parameters.iter().min_by(|a, b| {
            let da = a.fitness(c);
            let db = b.fitness(c);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }).map(|p| &p.biome).unwrap()
    }
}

impl BiomePoint {
    pub fn fitness(&self, c: &Climate) -> f32 {
        self.temperature.distance(c.temperature)
        + self.humidity.distance(c.humidity)
        + self.continentalness.distance(c.continentalness)
        + self.erosion.distance(c.erosion)
        + self.depth.distance(c.depth)
        + self.weirdness.distance(c.weirdness)
        + self.offset * self.offset
    }
}
```

### 26.6 — Aquifer placement

```rust
// crates/oxidized-game/src/worldgen/aquifer.rs

/// Handles per-column water/lava placement in caves and underground.
/// Mirrors `Aquifer` in Java.
pub struct Aquifer {
    /// World seed used to derive per-column water levels.
    pub seed: u64,
    /// Y below which lava replaces water in underground aquifer cells.
    pub lava_level: i32, // -54
}

impl Aquifer {
    pub const LAVA_LEVEL: i32 = -54;

    pub fn new(seed: u64) -> Self {
        Self { seed, lava_level: Self::LAVA_LEVEL }
    }

    /// Determine the fluid state at (x, y, z) given the final density.
    /// Returns None for solid blocks, Some(fluid) for air/fluid cells.
    pub fn compute_state(
        &self,
        x: i32, y: i32, z: i32,
        density: f64,
    ) -> Option<FluidState> {
        if density > 0.0 {
            // Solid block — aquifer doesn't apply
            return None;
        }
        // Below lava level → lava
        if y <= self.lava_level {
            return Some(FluidState::Lava);
        }
        // Check local water level from hashed grid cell
        let local_water_level = self.local_water_level(x, z);
        if y <= local_water_level {
            Some(FluidState::Water)
        } else {
            None // Air
        }
    }

    /// Derive a pseudo-random water level for the 16×16 aquifer cell containing (x, z).
    fn local_water_level(&self, x: i32, z: i32) -> i32 {
        let cell_x = x >> 4;
        let cell_z = z >> 4;
        let mut hash = self.seed
            .wrapping_mul(cell_x as u64 * 0x9E3779B97F4A7C15)
            .wrapping_add(cell_z as u64 * 0x6C62272E07BB0142);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xFF51AFD7ED558CCD);
        hash ^= hash >> 33;
        // Water level in range [-64, 0]
        -64 + (hash % 64) as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidState { Water, Lava }
```

### 26.7 — Surface rules

```rust
// crates/oxidized-game/src/worldgen/surface_rules.rs

use std::sync::Arc;

/// Context passed to every surface rule when evaluating a block column.
pub struct SurfaceContext<'a> {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub biome: &'a ResourceLocation,
    /// Blocks below surface (0 = topmost solid block, 1 = one below, …).
    pub depth_below_surface: i32,
    /// Y of the first water or air above.
    pub water_height: i32,
    pub is_under_water: bool,
    pub slope: bool,
}

pub trait SurfaceRule: Send + Sync {
    /// Returns the block state to place, or None if this rule does not apply.
    fn try_apply(&self, ctx: &SurfaceContext<'_>) -> Option<u32>; // block state id
}

/// Try each rule in order; return the first that produces a block.
pub struct SequenceRule(pub Vec<Arc<dyn SurfaceRule>>);
impl SurfaceRule for SequenceRule {
    fn try_apply(&self, ctx: &SurfaceContext<'_>) -> Option<u32> {
        self.0.iter().find_map(|r| r.try_apply(ctx))
    }
}

/// Apply inner rule only if the biome matches.
pub struct BiomeConditionRule {
    pub biomes: Vec<ResourceLocation>,
    pub inner:  Arc<dyn SurfaceRule>,
}
impl SurfaceRule for BiomeConditionRule {
    fn try_apply(&self, ctx: &SurfaceContext<'_>) -> Option<u32> {
        if self.biomes.iter().any(|b| b == ctx.biome) {
            self.inner.try_apply(ctx)
        } else {
            None
        }
    }
}

/// Apply inner rule only if y >= min (inclusive).
pub struct YAboveRule { pub min_y: i32, pub inner: Arc<dyn SurfaceRule> }
impl SurfaceRule for YAboveRule {
    fn try_apply(&self, ctx: &SurfaceContext<'_>) -> Option<u32> {
        if ctx.y >= self.min_y { self.inner.try_apply(ctx) } else { None }
    }
}

/// Place a constant block.
pub struct BlockRule(pub u32);
impl SurfaceRule for BlockRule {
    fn try_apply(&self, _: &SurfaceContext<'_>) -> Option<u32> { Some(self.0) }
}

/// depth_below_surface == 0 → grass; 1..=3 → dirt; else stone (overworld default).
pub fn overworld_surface_rules() -> Arc<dyn SurfaceRule> {
    Arc::new(SequenceRule(vec![
        // grass at surface
        Arc::new(DepthRule { max_depth: 0, inner: Arc::new(BlockRule(GRASS_BLOCK)) }),
        // dirt 1-3 deep
        Arc::new(DepthRule { max_depth: 3, inner: Arc::new(BlockRule(DIRT)) }),
        // stone below
        Arc::new(BlockRule(STONE)),
    ]))
}

pub struct DepthRule { pub max_depth: i32, pub inner: Arc<dyn SurfaceRule> }
impl SurfaceRule for DepthRule {
    fn try_apply(&self, ctx: &SurfaceContext<'_>) -> Option<u32> {
        if ctx.depth_below_surface <= self.max_depth { self.inner.try_apply(ctx) } else { None }
    }
}

const GRASS_BLOCK: u32 = 8; // placeholder block state id
const DIRT:        u32 = 9;
const STONE:       u32 = 1;
```

### 26.8 — Cave carver (worm algorithm)

```rust
// crates/oxidized-game/src/worldgen/carver/cave.rs

/// Cave carver using the worm algorithm.
/// Matches `CaveCarver` in Java.
pub struct CaveCarver {
    pub y_scale: f32,
    pub min_y:   i32,
    pub max_y:   i32,
    /// Radius multiplier range [1.5, 4.0].
    pub radius_min: f32,
    pub radius_max: f32,
}

impl CaveCarver {
    pub fn new() -> Self {
        Self { y_scale: 1.0, min_y: 8, max_y: 180, radius_min: 1.5, radius_max: 4.0 }
    }

    /// Carve caves into `carved_blocks` (a set of positions to be set to air).
    /// Each cave is a "worm": a series of overlapping ellipsoid cavities.
    pub fn carve(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        seed: u64,
        carved_blocks: &mut std::collections::HashSet<glam::IVec3>,
    ) {
        let mut rng = XoroshiroRandomSource::from_seed(seed as i64);
        // Number of worm starts per chunk: rng.next_int_bounded(rng.next_int_bounded(rng.next_int_bounded(15) + 1) + 1)
        let count = {
            let a = rng.next_int_bounded(15) + 1;
            let b = rng.next_int_bounded(a) + 1;
            rng.next_int_bounded(b) + 1
        };
        for _ in 0..count {
            self.carve_worm(chunk_x, chunk_z, &mut rng, carved_blocks);
        }
    }

    fn carve_worm(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        rng: &mut XoroshiroRandomSource,
        carved_blocks: &mut std::collections::HashSet<glam::IVec3>,
    ) {
        let mut x = (chunk_x * 16 + rng.next_int_bounded(16)) as f64;
        let mut y = (self.min_y + rng.next_int_bounded(self.max_y - self.min_y)) as f64;
        let mut z = (chunk_z * 16 + rng.next_int_bounded(16)) as f64;
        let mut yaw:   f64 = rng.next_f64() * std::f64::consts::TAU;
        let mut pitch: f64 = (rng.next_f64() - 0.5) * std::f64::consts::FRAC_PI_4;
        let radius = self.radius_min as f64
            + rng.next_f64() * (self.radius_max - self.radius_min) as f64;
        let segments = 112 + rng.next_int_bounded(16) as i32;

        for _ in 0..segments {
            x += yaw.sin() * pitch.cos();
            y += pitch.sin();
            z += yaw.cos() * pitch.cos();
            yaw   += (rng.next_f64() - 0.5) * 0.2;
            pitch *= 0.9;
            pitch += (rng.next_f64() - 0.5) * 0.5;
            let r = radius * (rng.next_f64() * 0.25 + 0.75);
            let ry = r * self.y_scale as f64;
            // Carve ellipsoid (r, ry, r) centered at (x, y, z)
            for bx in (x - r) as i32 ..= (x + r) as i32 {
                for by in (y - ry) as i32 ..= (y + ry) as i32 {
                    for bz in (z - r) as i32 ..= (z + r) as i32 {
                        let dx = (bx as f64 - x) / r;
                        let dy = (by as f64 - y) / ry;
                        let dz = (bz as f64 - z) / r;
                        if dx*dx + dy*dy + dz*dz < 1.0 {
                            carved_blocks.insert(glam::IVec3::new(bx, by, bz));
                        }
                    }
                }
            }
        }
    }
}
```

### 26.9 — `NoiseBasedChunkGenerator` pipeline and async dispatch

```rust
// crates/oxidized-game/src/worldgen/chunk_generator.rs

use tokio::task;

/// Full overworld chunk generation pipeline.
/// All work runs in a `tokio::task::spawn_blocking` thread to avoid
/// blocking the async runtime.
pub struct NoiseBasedChunkGenerator {
    pub seed: i64,
    pub biome_source: Arc<MultiNoiseBiomeSource>,
    pub final_density: Arc<dyn DensityFunction>,
    pub surface_rules: Arc<dyn SurfaceRule>,
    pub aquifer: Arc<Aquifer>,
    pub cave_carver: Arc<CaveCarver>,
}

impl NoiseBasedChunkGenerator {
    /// Generate a full chunk asynchronously.
    pub async fn generate_chunk(
        self: Arc<Self>,
        chunk_x: i32,
        chunk_z: i32,
    ) -> LevelChunk {
        let gen = self.clone();
        task::spawn_blocking(move || {
            gen.generate_chunk_sync(chunk_x, chunk_z)
        }).await.expect("chunk generation panicked")
    }

    /// Synchronous generation — called inside spawn_blocking.
    fn generate_chunk_sync(&self, chunk_x: i32, chunk_z: i32) -> LevelChunk {
        // Step 1: Build NoiseChunk (coarse density grid + trilinear interpolation).
        let noise_chunk = NoiseChunk::new(
            chunk_x, chunk_z, -64, 384, self.final_density.clone(),
        );

        // Step 2: Fill blocks using density and aquifer.
        let mut chunk = LevelChunk::new_empty(chunk_x, chunk_z, BiomeId(1));
        for lx in 0..16_usize {
            for lz in 0..16_usize {
                for ly in 0..384_usize {
                    let world_y = -64 + ly as i32;
                    let density = noise_chunk.density_at_block(lx, ly, lz);
                    let world_x = chunk_x * 16 + lx as i32;
                    let world_z = chunk_z * 16 + lz as i32;
                    let block_state = if density > 0.0 {
                        STONE_STATE
                    } else {
                        match self.aquifer.compute_state(world_x, world_y, world_z, density) {
                            Some(FluidState::Water) => WATER_STATE,
                            Some(FluidState::Lava)  => LAVA_STATE,
                            None => AIR_STATE,
                        }
                    };
                    chunk.set_block_state(BlockPos::new(world_x, world_y, world_z), block_state);
                }
            }
        }

        // Step 3: Apply surface rules (grass/dirt/stone/sand).
        self.apply_surface_rules(&mut chunk, chunk_x, chunk_z);

        // Step 4: Apply cave carver.
        let mut carved = std::collections::HashSet::new();
        self.cave_carver.carve(chunk_x, chunk_z, self.seed as u64, &mut carved);
        for pos in carved {
            chunk.set_block_state(BlockPos::new(pos.x, pos.y, pos.z), AIR_STATE);
        }

        // Step 5: Place features (ores, springs, trees — stubbed).
        self.place_features(&mut chunk, chunk_x, chunk_z);

        // Step 6: Recalculate heightmaps.
        chunk.recalculate_heightmaps();

        // Step 7: Assign biomes.
        self.assign_biomes(&mut chunk, chunk_x, chunk_z);

        chunk
    }

    fn apply_surface_rules(&self, chunk: &mut LevelChunk, cx: i32, cz: i32) {
        // Walk each column top-down, count depth below first solid, apply rules.
        for lx in 0..16_usize {
            for lz in 0..16_usize {
                let wx = cx * 16 + lx as i32;
                let wz = cz * 16 + lz as i32;
                let biome = self.biome_source.get_biome(wx, 0, wz);
                let mut depth = -1i32;
                for ly in (0..384_usize).rev() {
                    let wy = -64 + ly as i32;
                    let pos = BlockPos::new(wx, wy, wz);
                    let state = chunk.get_block_state(pos).id;
                    if state == STONE_STATE.id {
                        depth += 1;
                        let water_height = 62;
                        let ctx = SurfaceContext {
                            x: wx, y: wy, z: wz, biome,
                            depth_below_surface: depth,
                            water_height,
                            is_under_water: wy < water_height,
                            slope: false,
                        };
                        if let Some(new_state) = self.surface_rules.try_apply(&ctx) {
                            chunk.set_block_state(pos, BlockState { id: new_state });
                        }
                    } else {
                        depth = -1;
                    }
                }
            }
        }
    }

    fn place_features(&self, _chunk: &mut LevelChunk, _cx: i32, _cz: i32) {
        // Ore veins, water/lava springs, disk features, tree placement.
        // Stubbed; each feature type is a separate decorator.
    }

    fn assign_biomes(&self, chunk: &mut LevelChunk, cx: i32, cz: i32) {
        // Sample biome at each 4×4×4 biome cell and write into section palettes.
    }
}

// Placeholder block-state constants
const AIR_STATE:   BlockState = BlockState { id: 0 };
const STONE_STATE: BlockState = BlockState { id: 1 };
const WATER_STATE: BlockState = BlockState { id: 34 };
const LAVA_STATE:  BlockState = BlockState { id: 50 };
```

---

## Data Structures Summary

```rust
// Key types in oxidized-game::worldgen

pub use noise::perlin::{ImprovedNoise, PerlinNoise};
pub use noise::normal_noise::NormalNoise;
pub use noise::simplex::SimplexNoise;
pub use random::{XoroshiroRandomSource, RngSource, PositionalRandomFactory};
pub use density_function::{DensityFunction, DfRef, FunctionContext,
                            ConstantDf, NoiseDf, AddDf, MulDf, ClampDf,
                            YClampedGradientDf, FlatCacheDf, InterpolatedDf};
pub use noise_chunk::NoiseChunk;
pub use biome_source::{MultiNoiseBiomeSource, Climate, BiomePoint, ParameterRange};
pub use aquifer::{Aquifer, FluidState};
pub use surface_rules::{SurfaceRule, SurfaceContext, SequenceRule,
                         BiomeConditionRule, YAboveRule, BlockRule};
pub use carver::cave::CaveCarver;
pub use chunk_generator::NoiseBasedChunkGenerator;
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::random::XoroshiroRandomSource;
    use crate::worldgen::noise_chunk::NoiseChunk;
    use crate::worldgen::density_function::{ConstantDf, YClampedGradientDf, FunctionContext};
    use std::sync::Arc;

    // --- XoroshiroRandomSource ---

    /// Output is deterministic for the same seed.
    #[test]
    fn xoroshiro_deterministic() {
        let mut a = XoroshiroRandomSource::from_seed(12345);
        let mut b = XoroshiroRandomSource::from_seed(12345);
        for _ in 0..100 {
            assert_eq!(a.next_long(), b.next_long());
        }
    }

    /// next_int_bounded returns values in [0, bound).
    #[test]
    fn xoroshiro_bounded_range() {
        let mut rng = XoroshiroRandomSource::from_seed(0xDEADBEEF);
        for _ in 0..10_000 {
            let v = rng.next_int_bounded(17);
            assert!((0..17).contains(&v));
        }
    }

    /// next_f64 returns values in [0.0, 1.0).
    #[test]
    fn xoroshiro_f64_range() {
        let mut rng = XoroshiroRandomSource::from_seed(777);
        for _ in 0..10_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
        }
    }

    // --- ConstantDf ---

    #[test]
    fn constant_df_returns_constant() {
        let df = ConstantDf(3.14);
        let v = df.compute(FunctionContext { block_x: 0, block_y: 0, block_z: 0 });
        assert!((v - 3.14).abs() < 1e-10);
    }

    // --- YClampedGradientDf ---

    #[test]
    fn y_gradient_clamps_at_extremes() {
        let df = YClampedGradientDf { from_y: 0, to_y: 100, from_v: 0.0, to_v: 1.0 };
        let at_min = df.compute(FunctionContext { block_x: 0, block_y: -10, block_z: 0 });
        let at_max = df.compute(FunctionContext { block_x: 0, block_y: 200, block_z: 0 });
        assert!((at_min - 0.0).abs() < 1e-10);
        assert!((at_max - 1.0).abs() < 1e-10);
    }

    #[test]
    fn y_gradient_midpoint() {
        let df = YClampedGradientDf { from_y: 0, to_y: 100, from_v: 0.0, to_v: 1.0 };
        let mid = df.compute(FunctionContext { block_x: 0, block_y: 50, block_z: 0 });
        assert!((mid - 0.5).abs() < 1e-10);
    }

    // --- NoiseChunk trilinear ---

    /// Constant density function → every block returns the same value.
    #[test]
    fn noise_chunk_constant_density() {
        let df: Arc<dyn DensityFunction> = Arc::new(ConstantDf(0.5));
        let nc = NoiseChunk::new(0, 0, -64, 384, df);
        for lx in [0, 8, 15] {
            for ly in [0, 100, 383] {
                for lz in [0, 8, 15] {
                    let d = nc.density_at_block(lx, ly, lz);
                    assert!((d - 0.5).abs() < 1e-9,
                        "Expected 0.5 at ({lx},{ly},{lz}), got {d}");
                }
            }
        }
    }

    // --- ParameterRange distance ---

    #[test]
    fn parameter_range_inside_is_zero() {
        let r = ParameterRange::span(-0.5, 0.5);
        assert_eq!(r.distance(0.0), 0.0);
        assert_eq!(r.distance(0.5), 0.0);
    }

    #[test]
    fn parameter_range_outside_is_positive() {
        let r = ParameterRange::span(0.0, 1.0);
        let d = r.distance(2.0);
        assert!((d - 1.0).abs() < 1e-6);
    }

    // --- Aquifer ---

    #[test]
    fn aquifer_lava_below_y_neg54() {
        let aq = Aquifer::new(42);
        let state = aq.compute_state(0, Aquifer::LAVA_LEVEL - 1, 0, -1.0);
        assert_eq!(state, Some(FluidState::Lava));
    }

    #[test]
    fn aquifer_solid_block_returns_none() {
        let aq = Aquifer::new(42);
        // density > 0 → solid → aquifer doesn't apply
        assert_eq!(aq.compute_state(0, 64, 0, 1.0), None);
    }
}
```
