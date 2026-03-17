# Phase 5 — NBT Implementation

**Crate:** `oxidized-nbt`  
**Reward:** Read and write any Minecraft `.dat`, `.mca`, or `.nbt` file correctly.

---

## Goal

Full implementation of the Named Binary Tag (NBT) format used by Minecraft for
chunk data, player data, level metadata, and structure files.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Tag types | `net.minecraft.nbt.Tag` (interface) |
| Compound | `net.minecraft.nbt.CompoundTag` |
| List | `net.minecraft.nbt.ListTag` |
| Numeric tags | `net.minecraft.nbt.ByteTag`, `ShortTag`, `IntTag`, `LongTag`, `FloatTag`, `DoubleTag` |
| Array tags | `net.minecraft.nbt.ByteArrayTag`, `IntArrayTag`, `LongArrayTag` |
| String tag | `net.minecraft.nbt.StringTag` |
| I/O | `net.minecraft.nbt.NbtIo` |
| SNBT | `net.minecraft.nbt.TagParser` |
| Memory limit | `net.minecraft.nbt.NbtAccounter` |

---

## Tag Types

| ID | Type | Java | Rust |
|----|------|------|------|
| 0 | End | `EndTag` | `NbtTag::End` |
| 1 | Byte | `ByteTag` | `NbtTag::Byte(i8)` |
| 2 | Short | `ShortTag` | `NbtTag::Short(i16)` |
| 3 | Int | `IntTag` | `NbtTag::Int(i32)` |
| 4 | Long | `LongTag` | `NbtTag::Long(i64)` |
| 5 | Float | `FloatTag` | `NbtTag::Float(f32)` |
| 6 | Double | `DoubleTag` | `NbtTag::Double(f64)` |
| 7 | ByteArray | `ByteArrayTag` | `NbtTag::ByteArray(Vec<i8>)` |
| 8 | String | `StringTag` | `NbtTag::String(String)` |
| 9 | List | `ListTag` | `NbtTag::List(NbtList)` |
| 10 | Compound | `CompoundTag` | `NbtTag::Compound(NbtCompound)` |
| 11 | IntArray | `IntArrayTag` | `NbtTag::IntArray(Vec<i32>)` |
| 12 | LongArray | `LongArrayTag` | `NbtTag::LongArray(Vec<i64>)` |

---

## Binary Format

```
Named tag:
  [tag_type: u8]
  [name_length: u16 big-endian]
  [name: utf8 bytes]
  [payload]

Payload by type:
  Byte:      [i8]
  Short:     [i16 big-endian]
  Int:       [i32 big-endian]
  Long:      [i64 big-endian]
  Float:     [f32 big-endian]
  Double:    [f64 big-endian]
  ByteArray: [length: i32][bytes...]
  String:    [length: u16][utf8 bytes...]  (Modified UTF-8!)
  List:      [element_type: u8][length: i32][payload × length]
  Compound:  [named tag...][tag_type=0 (End)]
  IntArray:  [length: i32][i32 × length]
  LongArray: [length: i32][i64 × length]

Root compound: named tag (name usually "" for file root)
```

**Important:** NBT Strings use **Modified UTF-8**, same as Java's `DataOutputStream.writeUTF()`.
Null bytes in strings are encoded as `[0xC0, 0x80]` instead of `[0x00]`.

---

## Tasks

### 5.1 — Core Tag Type (`src/tag.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(NbtList),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn type_id(&self) -> u8;
    pub fn type_name(&self) -> &'static str;
    // Convenience accessors
    pub fn as_byte(&self) -> Option<i8>;
    pub fn as_int(&self) -> Option<i32>;
    pub fn as_long(&self) -> Option<i64>;
    pub fn as_float(&self) -> Option<f32>;
    pub fn as_double(&self) -> Option<f64>;
    pub fn as_str(&self) -> Option<&str>;
    pub fn as_compound(&self) -> Option<&NbtCompound>;
    pub fn as_list(&self) -> Option<&NbtList>;
}
```

### 5.2 — Compound and List (`src/compound.rs`, `src/list.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NbtCompound {
    entries: IndexMap<String, NbtTag>,  // preserve insertion order
}

impl NbtCompound {
    pub fn new() -> Self;
    pub fn get(&self, key: &str) -> Option<&NbtTag>;
    pub fn get_byte(&self, key: &str) -> Option<i8>;
    pub fn get_int(&self, key: &str) -> Option<i32>;
    pub fn get_long(&self, key: &str) -> Option<i64>;
    pub fn get_string(&self, key: &str) -> Option<&str>;
    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound>;
    pub fn get_list(&self, key: &str) -> Option<&NbtList>;
    pub fn put(&mut self, key: impl Into<String>, value: NbtTag);
    pub fn put_byte(&mut self, key: &str, value: i8);
    pub fn put_int(&mut self, key: &str, value: i32);
    // ... etc
    pub fn contains_key(&self, key: &str) -> bool;
    pub fn remove(&mut self, key: &str) -> Option<NbtTag>;
    pub fn iter(&self) -> impl Iterator<Item = (&String, &NbtTag)>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct NbtList {
    element_type: u8,
    elements: Vec<NbtTag>,
}

impl NbtList {
    pub fn new(element_type: u8) -> Self;
    pub fn element_type(&self) -> u8;
    pub fn get(&self, index: usize) -> Option<&NbtTag>;
    pub fn add(&mut self, tag: NbtTag) -> Result<(), NbtError>;  // type check
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = &NbtTag>;
    pub fn compounds(&self) -> impl Iterator<Item = &NbtCompound>;
}
```

### 5.3 — Binary Reader (`src/io/read.rs`)

```rust
pub struct NbtReader<R: Read> {
    inner: R,
    accounter: NbtAccounter,
}

impl<R: Read> NbtReader<R> {
    pub fn read_named_tag(&mut self) -> Result<(String, NbtTag), NbtError>;
    pub fn read_payload(&mut self, type_id: u8) -> Result<NbtTag, NbtError>;
    pub fn read_compound(&mut self) -> Result<NbtCompound, NbtError>;
    pub fn read_list(&mut self) -> Result<NbtList, NbtError>;
    // Read modified UTF-8 string
    fn read_string(&mut self) -> Result<String, NbtError>;
}

pub struct NbtAccounter {
    used: usize,
    max: usize,  // default 67_108_864 (64 MiB)
}

impl NbtAccounter {
    pub fn account_for(&mut self, bytes: usize) -> Result<(), NbtError>;
}
```

### 5.4 — Binary Writer (`src/io/write.rs`)

```rust
pub struct NbtWriter<W: Write> {
    inner: W,
}

impl<W: Write> NbtWriter<W> {
    pub fn write_named_tag(&mut self, name: &str, tag: &NbtTag) -> Result<(), NbtError>;
    pub fn write_compound(&mut self, compound: &NbtCompound) -> Result<(), NbtError>;
    pub fn write_payload(&mut self, tag: &NbtTag) -> Result<(), NbtError>;
}
```

### 5.5 — File I/O Helpers (`src/io/file.rs`)

```rust
/// Read NBT from a GZIP-compressed file (e.g. level.dat, player .dat files)
pub fn read_gzip_file(path: &Path) -> Result<NbtCompound, NbtError>;

/// Read NBT from a zlib-compressed buffer (e.g. chunk data in .mca)
pub fn read_zlib_bytes(data: &[u8]) -> Result<NbtCompound, NbtError>;

/// Read uncompressed NBT bytes
pub fn read_bytes(data: &[u8]) -> Result<NbtCompound, NbtError>;

/// Write NBT to a GZIP-compressed file
pub fn write_gzip_file(path: &Path, tag: &NbtCompound) -> Result<(), NbtError>;

/// Write NBT to a zlib-compressed buffer
pub fn write_zlib_bytes(tag: &NbtCompound) -> Result<Vec<u8>, NbtError>;
```

### 5.6 — SNBT (String NBT) (`src/snbt.rs`)

SNBT is the human-readable NBT format used in commands and data packs.

```
Examples:
  {key: 42, name: "hello", list: [1, 2, 3]}
  42b         → ByteTag(42)
  42s         → ShortTag(42)
  42L         → LongTag(42)
  42.0f       → FloatTag(42.0)
  42.0d       → DoubleTag(42.0)
  42.0        → DoubleTag(42.0)
  [B; 1b, 2b] → ByteArrayTag
  [I; 1, 2]   → IntArrayTag
  [L; 1L, 2L] → LongArrayTag
```

```rust
pub fn parse_snbt(input: &str) -> Result<NbtTag, SnbtError>;
pub fn format_snbt(tag: &NbtTag) -> String;
pub fn format_snbt_pretty(tag: &NbtTag, indent: usize) -> String;
```

### 5.7 — Serde integration (`src/serde.rs`)

Allow Rust structs with `#[derive(Serialize, Deserialize)]` to be read/written as NBT:

```rust
pub fn from_compound<T: DeserializeOwned>(tag: &NbtCompound) -> Result<T, NbtError>;
pub fn to_compound<T: Serialize>(value: &T) -> Result<NbtCompound, NbtError>;
```

---

## Tests

```rust
#[test]
fn test_roundtrip_compound() { /* write compound, read back, compare */ }

#[test]
fn test_read_level_dat() {
    // Load mc-server-ref test world level.dat
    // Verify "Data.LevelName" exists
}

#[test]
fn test_snbt_parse_int()       { assert_eq!(parse_snbt("42").unwrap(), NbtTag::Int(42)); }
#[test]
fn test_snbt_parse_byte()      { assert_eq!(parse_snbt("42b").unwrap(), NbtTag::Byte(42)); }
#[test]
fn test_snbt_parse_compound()  { parse_snbt("{key: 1, name: \"hello\"}").unwrap(); }
#[test]
fn test_snbt_parse_list()      { parse_snbt("[1, 2, 3]").unwrap(); }
#[test]
fn test_snbt_roundtrip()       { /* parse → format → parse and compare */ }

#[test]
fn test_modified_utf8_null_byte() {
    // String containing null byte round-trips correctly
}
```
