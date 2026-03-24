# Phase 7 — Core Data Types

**Status:** ✅ Complete  
**Crate:** `oxidized-world`  
**Reward:** All foundational spatial and game-state types are implemented and
tested — the building blocks every other system depends on.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-013: Coordinate Types](../adr/adr-013-coordinate-types.md) — newtype wrappers preventing coordinate mix-ups


## Goal

Implement the fundamental Minecraft coordinate, geometry, and game-state types
that all other crates will import.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Block position | `net.minecraft.core.BlockPos` |
| Chunk position | `net.minecraft.world.level.ChunkPos` |
| Section position | `net.minecraft.core.SectionPos` |
| 3D float vector | `net.minecraft.world.phys.Vec3` |
| Integer 3D | `net.minecraft.core.Vec3i` |
| 2D float | `net.minecraft.world.phys.Vec2` |
| AABB | `net.minecraft.world.phys.AABB` |
| Direction | `net.minecraft.core.Direction` |
| Game type | `net.minecraft.world.level.GameType` |
| Difficulty | `net.minecraft.world.Difficulty` |
| Resource location | `net.minecraft.resources.ResourceLocation` |
| Rotations | `net.minecraft.core.Rotations` |

---

## Tasks

### 7.1 — `ResourceLocation` (`src/types/resource_location.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub namespace: String,
    pub path: String,
}

impl ResourceLocation {
    pub fn new(namespace: &str, path: &str) -> Self;
    pub fn minecraft(path: &str) -> Self;  // "minecraft:" prefix
    pub fn parse(input: &str) -> Result<Self, ParseError>;  // "namespace:path"
}

impl Display for ResourceLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl FromStr for ResourceLocation {
    // "path" → ResourceLocation { namespace: "minecraft", path: "path" }
    // "ns:path" → ResourceLocation { namespace: "ns", path: "path" }
}
```

### 7.2 — `BlockPos` (`src/types/block_pos.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: BlockPos;
    pub fn new(x: i32, y: i32, z: i32) -> Self;
    
    /// Minecraft packed format: x in bits [63:38], z in bits [37:12], y in bits [11:0]
    pub fn from_packed(packed: i64) -> Self;
    pub fn to_packed(self) -> i64;
    
    pub fn offset(self, dir: Direction) -> Self;
    pub fn above(self) -> Self;
    pub fn below(self) -> Self;
    pub fn north(self) -> Self;
    pub fn south(self) -> Self;
    pub fn east(self) -> Self;
    pub fn west(self) -> Self;
    
    pub fn chunk_pos(self) -> ChunkPos;
    pub fn section_pos(self) -> SectionPos;
    pub fn in_chunk_local_x(self) -> usize;  // 0..16
    pub fn in_chunk_local_y(self) -> usize;  // 0..16
    pub fn in_chunk_local_z(self) -> usize;  // 0..16
    pub fn section_index(self, min_y: i32) -> usize;
    
    pub fn distance_sq(self, other: BlockPos) -> f64;
}

// Packed format (from BlockPos.java):
//   x = (packed << 64-26 >> 64-26)  ... 26 bits for x
//   z = (packed << 64-52 >> 64-26)  ... 26 bits for z
//   y = (packed << 64-12 >> 64-12)  ... 12 bits for y
// Actually: x>>38, z & 0xFFFFFFF then sign extend, y & 0xFFF then sign extend
```

### 7.3 — `ChunkPos` (`src/types/chunk_pos.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub fn new(x: i32, z: i32) -> Self;
    pub fn from_block_pos(pos: BlockPos) -> Self;
    pub fn to_long(self) -> i64;  // z << 32 | x (as unsigned)
    pub fn from_long(l: i64) -> Self;
    pub fn min_block_x(self) -> i32;  // x * 16
    pub fn min_block_z(self) -> i32;
    pub fn max_block_x(self) -> i32;  // x * 16 + 15
    pub fn max_block_z(self) -> i32;
    pub fn chebyshev_distance(self, other: ChunkPos) -> i32;
    
    /// Iterate all chunk positions within `radius` (Chebyshev) — spiral order (center first).
    pub fn spiral_from(center: ChunkPos, radius: i32) -> impl Iterator<Item = ChunkPos>;
}
```

### 7.4 — `SectionPos` (`src/types/section_pos.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionPos {
    pub fn from_block_pos(pos: BlockPos) -> Self;
    pub fn chunk_pos(self) -> ChunkPos;
    pub fn min_block_pos(self) -> BlockPos;
    pub fn section_index_for_y(world_y: i32, min_section_y: i32) -> usize;
}
```

### 7.5 — `Vec3` (`src/types/vec3.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3;
    pub fn new(x: f64, y: f64, z: f64) -> Self;
    pub fn length(self) -> f64;
    pub fn length_sq(self) -> f64;
    pub fn normalize(self) -> Self;
    pub fn dot(self, other: Vec3) -> f64;
    pub fn cross(self, other: Vec3) -> Vec3;
    pub fn lerp(self, other: Vec3, t: f64) -> Vec3;
    pub fn add(self, x: f64, y: f64, z: f64) -> Vec3;
    pub fn scale(self, factor: f64) -> Vec3;
    pub fn distance_to(self, other: Vec3) -> f64;
    pub fn to_block_pos(self) -> BlockPos;
    /// Yaw and pitch from direction vector
    pub fn to_yaw_pitch(self) -> (f32, f32);
    /// Direction vector from yaw+pitch
    pub fn from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3;
}

impl Add<Vec3> for Vec3 { type Output = Vec3; }
impl Sub<Vec3> for Vec3 { type Output = Vec3; }
impl Mul<f64> for Vec3 { type Output = Vec3; }
```

### 7.6 — `AABB` (`src/types/aabb.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min_x: f64, pub min_y: f64, pub min_z: f64,
    pub max_x: f64, pub max_y: f64, pub max_z: f64,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self;
    pub fn of(x: f64, y: f64, z: f64, w: f64, h: f64, d: f64) -> Self;
    pub fn center(self) -> Vec3;
    pub fn size(self) -> Vec3;
    pub fn intersects(self, other: AABB) -> bool;
    pub fn contains(self, v: Vec3) -> bool;
    pub fn expand(self, x: f64, y: f64, z: f64) -> AABB;
    pub fn inflate(self, amount: f64) -> AABB;
    pub fn move_by(self, delta: Vec3) -> AABB;
    pub fn clip(self, from: Vec3, to: Vec3) -> Option<Vec3>;  // ray-AABB intersection
}
```

### 7.7 — `Direction` (`src/types/direction.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Direction {
    pub const ALL: [Direction; 6];
    pub fn opposite(self) -> Direction;
    pub fn axis(self) -> Axis;
    pub fn normal(self) -> Vec3i;
    pub fn step_x(self) -> i32;
    pub fn step_y(self) -> i32;
    pub fn step_z(self) -> i32;
    pub fn from_yaw(yaw: f32) -> Direction;  // horizontal only
    pub fn horizontal() -> impl Iterator<Item = Direction>;
    pub fn shuffled() -> impl Iterator<Item = Direction>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis { X, Y, Z }
```

### 7.8 — Game Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum GameType {
    Survival  = 0,
    Creative  = 1,
    Adventure = 2,
    Spectator = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum Difficulty {
    Peaceful = 0,
    Easy     = 1,
    Normal   = 2,
    Hard     = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanoidArm { Left, Right }
```

---

## Tests

```rust
#[test]
fn test_block_pos_packed_roundtrip() {
    for pos in [(0,0,0), (100, 64, -200), (-1, -64, -1), (i32::MAX>>6, 2047, i32::MAX>>6)] {
        let p = BlockPos::new(pos.0, pos.1, pos.2);
        assert_eq!(BlockPos::from_packed(p.to_packed()), p);
    }
}
#[test]
fn test_chunk_pos_from_block() {
    assert_eq!(ChunkPos::from_block_pos(BlockPos::new(15, 64, 15)), ChunkPos::new(0, 0));
    assert_eq!(ChunkPos::from_block_pos(BlockPos::new(16, 64, 16)), ChunkPos::new(1, 1));
    assert_eq!(ChunkPos::from_block_pos(BlockPos::new(-1, 64, -1)), ChunkPos::new(-1, -1));
}
#[test]
fn test_vec3_normalize() { /* length should be ~1.0 after normalize */ }
#[test]
fn test_aabb_intersects() { /* overlapping and non-overlapping boxes */ }
#[test]
fn test_resource_location_parse() {
    assert_eq!(ResourceLocation::parse("minecraft:stone").unwrap().to_string(), "minecraft:stone");
    assert_eq!(ResourceLocation::parse("stone").unwrap().namespace, "minecraft");
}
#[test]
fn test_chunk_pos_spiral() {
    let positions: Vec<_> = ChunkPos::spiral_from(ChunkPos::new(0, 0), 1).collect();
    assert_eq!(positions[0], ChunkPos::new(0, 0));  // center first
    assert_eq!(positions.len(), 9);  // 3×3
}
```
