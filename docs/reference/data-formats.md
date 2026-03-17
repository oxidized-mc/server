# Binary Formats Reference

All binary formats used by Minecraft's Java Edition protocol and world storage.
All values are big-endian unless explicitly marked LE.

---

## 1. VarInt / VarLong

Variable-length integer encoding. Each byte contributes 7 data bits; bit 7
(MSB) is the "continuation" flag — set means more bytes follow.

### Encoding algorithm

```
while value > 0x7F:
    write_byte((value & 0x7F) | 0x80)
    value >>= 7
write_byte(value & 0x7F)
```

For signed values, the sign bit is encoded in the low-order bits (VarInt treats
the value as unsigned u32 before encoding; negative i32 values become large
u32 values and thus always use 5 bytes).

### VarInt examples

| Value (decimal) | Hex bytes | Notes |
|---|---|---|
| `0` | `00` | 1 byte |
| `1` | `01` | 1 byte |
| `127` | `7F` | 1 byte (max 1-byte) |
| `128` | `80 01` | 2 bytes; 0x80 = continuation+0, 0x01 = high bits |
| `255` | `FF 01` | 2 bytes |
| `2097151` | `FF FF 7F` | 3 bytes (max 3-byte) |
| `2147483647` | `FF FF FF FF 07` | 5 bytes (i32::MAX) |
| `-1` | `FF FF FF FF 0F` | 5 bytes; -1 as two's complement u32 = 0xFFFFFFFF |
| `-2147483648` | `80 80 80 80 08` | 5 bytes (i32::MIN) |

### VarLong examples

| Value (decimal) | Hex bytes |
|---|---|
| `0` | `00` |
| `127` | `7F` |
| `128` | `80 01` |
| `2097151` | `FF FF 7F` |
| `9223372036854775807` | `FF FF FF FF FF FF FF FF 7F` (9 bytes, i64::MAX) |
| `-1` | `FF FF FF FF FF FF FF FF FF 01` (10 bytes) |

### Maximum lengths

| Type | Max bytes | Max value |
|---|---|---|
| VarInt | 5 | i32::MAX or u32::MAX depending on context |
| VarLong | 10 | i64::MAX or u64::MAX |

---

## 2. Packet Frame Format

Every packet on the wire (after connection handshake and optional compression /
encryption) is wrapped in two layers:

```
 ┌─────────────────────────────────────────────────┐
 │  Outer frame (always present)                   │
 │  ┌─────────────────────────────────────────────┐│
 │  │ Packet Length   VarInt  (bytes after here)  ││
 │  ├─────────────────────────────────────────────┤│
 │  │ [Compression frame, if enabled]             ││
 │  │  ┌───────────────────────────────────────┐  ││
 │  │  │ Data Length  VarInt  (0 = uncompressed)│  ││
 │  │  ├───────────────────────────────────────┤  ││
 │  │  │ Payload      bytes                    │  ││
 │  │  └───────────────────────────────────────┘  ││
 │  │  [if Data Length > 0: zlib-deflate compressed]││
 │  └─────────────────────────────────────────────┘│
 └─────────────────────────────────────────────────┘
```

When compression is **disabled**, the outer frame contains just:

```
[VarInt: packet length][bytes: packet id (VarInt) + packet fields]
```

When compression is **enabled** (threshold set via `ClientboundSetCompressionPacket`):

```
[VarInt: outer_len]
  [VarInt: data_len]       ← 0 if this packet is below threshold (not compressed)
  [bytes: payload]         ← zlib-deflated if data_len > 0
```

AES/CFB8 encryption wraps the entire byte stream **after** framing.

---

## 3. Login Encryption Sequence

Step-by-step with exact byte-level detail for each message:

```
Client                                 Server
  │                                      │
  │─── ServerboundHelloPacket ──────────►│
  │    VarInt: 0x00 (packet id)          │
  │    String: username (max 16 chars)   │
  │    Optional[UUID]: player_uuid       │
  │                                      │ ← generate 1024-bit RSA key pair
  │                                      │ ← generate 4-byte verify_token
  │◄── ClientboundHelloPacket ───────────│
  │    VarInt: 0x01 (packet id)          │
  │    String: server_id (always "")     │
  │    ByteArray: public_key (DER/X509)  │
  │    ByteArray: verify_token (4 bytes) │
  │    Boolean: should_authenticate      │
  │                                      │
  │ ← generate 16-byte shared_secret (AES key)
  │ ← encrypt shared_secret with server RSA public key
  │ ← encrypt verify_token with server RSA public key
  │ ← hash = SHA1(server_id + shared_secret + public_key_bytes)
  │   Note: SHA1 digest is treated as a signed BigInteger, then hex with
  │         no leading zeros; negative values get a '-' prefix
  │ ← POST https://sessionserver.mojang.com/session/minecraft/join
  │     body: { selectedProfile, serverId (hash), accessToken }
  │                                      │
  │─── ServerboundKeyPacket ────────────►│
  │    VarInt: 0x01 (packet id)          │
  │    ByteArray: encrypted_shared_secret│  (RSA PKCS#1 v1.5)
  │    ByteArray: encrypted_verify_token │  (RSA PKCS#1 v1.5)
  │                                      │
  │                                      │ ← decrypt shared_secret with RSA private key
  │                                      │ ← decrypt verify_token; assert == original
  │                                      │ ← derive SHA1 hash (same formula)
  │                                      │ ← GET https://sessionserver.mojang.com/
  │                                      │       session/minecraft/hasJoined?
  │                                      │       username=<name>&serverId=<hash>
  │                                      │   → 200 OK with GameProfile JSON
  │                                      │ ← enable AES/CFB8 encryption both dirs
  │◄─ [all subsequent bytes are AES/CFB8 encrypted] ──
  │◄── ClientboundLoginFinishedPacket ───│
  │    VarInt: 0x02 (packet id)          │
  │    UUID: player uuid                 │
  │    String: username                  │
  │    Array: properties (skin, cape)    │
  │                                      │
  │─── ServerboundLoginAcknowledged ────►│
  │    VarInt: 0x03 (packet id)          │
  │                                      │
  │         [→ Configuration state]      │
```

**SHA1 "Minecraft hash" algorithm:**

```python
import hashlib, struct

def minecraft_hash(server_id: str, shared_secret: bytes, public_key: bytes) -> str:
    digest = hashlib.sha1()
    digest.update(server_id.encode('ascii'))
    digest.update(shared_secret)
    digest.update(public_key)
    raw = digest.digest()
    # Interpret as signed big-endian integer
    value = int.from_bytes(raw, 'big', signed=True)
    return format(value, 'x')  # hex, possibly with leading '-'
```

---

## 4. NBT Binary Format

Named Binary Tag (NBT) is the serialisation format for world data, entity data,
item data, and more.

### Tag type IDs

| ID | Tag name | Payload layout |
|----|---------|---------------|
| `0` | TAG_End | 0 bytes (terminates TAG_Compound) |
| `1` | TAG_Byte | 1 byte signed i8 |
| `2` | TAG_Short | 2 bytes signed i16 BE |
| `3` | TAG_Int | 4 bytes signed i32 BE |
| `4` | TAG_Long | 8 bytes signed i64 BE |
| `5` | TAG_Float | 4 bytes IEEE 754 f32 BE |
| `6` | TAG_Double | 8 bytes IEEE 754 f64 BE |
| `7` | TAG_Byte_Array | `[i32 BE: length][i8 × length]` |
| `8` | TAG_String | `[u16 BE: byte_len][MUTF-8 bytes × byte_len]` |
| `9` | TAG_List | `[u8: element_type_id][i32 BE: count][element × count]` |
| `10` | TAG_Compound | `[tag × …][TAG_End]` (each tag: type_id + name + payload) |
| `11` | TAG_Int_Array | `[i32 BE: length][i32 BE × length]` |
| `12` | TAG_Long_Array | `[i32 BE: length][i64 BE × length]` |

### Named tag wire format

```
[u8: type_id]
[u16 BE: name_byte_length]
[MUTF-8 bytes: name]
[payload bytes for this type_id]
```

Except for TAG_End, which has no name and no payload; just the single `0x00`
byte.

### Root compound (file on disk)

```
[u8: 0x0A]              ← TAG_Compound
[u16: 0x0000]           ← empty string (unnamed root)
[content tags...]
[u8: 0x00]              ← TAG_End
```

Level.dat and chunk data are typically GZip-compressed before storage.
Network NBT (item data, entity metadata) is uncompressed.

### Example — small compound

Encoding `{"Level": 8}`:

```
0A            ← TAG_Compound
00 00         ← root name = ""
03            ← TAG_Int (child)
00 05         ← name length = 5
4C 65 76 65 6C  ← "Level"
00 00 00 08   ← value = 8
00            ← TAG_End
```

---

## 5. Chunk Wire Format — PalettedContainer

Each chunk section sent in `ClientboundLevelChunkWithLightPacket` contains two
`PalettedContainer` payloads: one for block states and one for biomes.

### PalettedContainer binary layout

```
[u8: bits_per_entry]
[palette section]
[VarInt: data array length in longs]
[i64 × length: packed data]
```

### Palette section by type

#### Single Value (bits_per_entry = 0)

```
[VarInt: single_value_id]   ← global palette ID of the only value
[VarInt: 0]                 ← data array length = 0 (no data array)
```

#### Linear / HashMap palette (indirect, bits_per_entry = 1–8)

```
[VarInt: palette_length]
[VarInt × palette_length: global_palette_ids]
[VarInt: data_array_length_in_longs]
[i64 × data_array_length: packed indices]
```

#### Global palette (bits_per_entry ≥ 9 for blocks, ≥ 4 for biomes)

```
[VarInt: 0]                 ← no palette (values are direct global IDs)
[VarInt: data_array_length_in_longs]
[i64 × data_array_length: packed global IDs]
```

### Packed data layout

For `bits_per_entry = B`:

- Each `i64` holds `⌊64 / B⌋` values (no cross-long packing — values do NOT
  span two longs)
- Bits are stored from LSB to MSB within each long
- Padding bits at the top of each long are zeroed
- Total data longs = `⌈(4096 / ⌊64 / B⌋)⌉` for block sections

```
bits_per_entry = 4, 16 values per i64:
 i64[0]:  index 0 in bits [0:3], index 1 in bits [4:7], ..., index 15 in bits [60:63]
 i64[1]:  index 16 in bits [0:3], ...
```

### Thresholds (blocks)

| bits_per_entry | Palette type | Max distinct values |
|---|---|---|
| 0 | Single value | 1 |
| 1–4 | Linear | 16 |
| 5–8 | HashMap | 256 |
| 4096 states (≥9) | Global | unlimited |

### Thresholds (biomes)

| bits_per_entry | Palette type |
|---|---|
| 0 | Single value |
| 1–3 | Linear |
| ≥4 | Global |

---

## 6. Region File (.mca) Format

Region files store 32×32 chunks. Each file is named `r.<rx>.<rz>.mca` where
`rx = floor(chunk_x / 32)`.

### File header (8192 bytes)

```
 Bytes 0–4095:    Location table (1024 × 4 bytes)
 Bytes 4096–8191: Timestamp table (1024 × 4 bytes)
```

### Location table entry (4 bytes each, 1024 entries)

```
 [u24 BE: sector_offset]  ← sector (4096-byte page) index; 0 = not present
 [u8:  sector_count]      ← number of 4096-byte sectors occupied
```

Chunk index within a region: `(chunk_x & 31) + (chunk_z & 31) × 32`

### Timestamp table entry (4 bytes)

```
 [u32 BE: unix_timestamp]  ← seconds since epoch; when chunk was last modified
```

### Chunk data (at sector_offset × 4096)

```
 [u32 BE: byte_length]      ← length of (compression_type + compressed_data)
 [u8:  compression_type]    ← 1=GZip, 2=Zlib, 3=Uncompressed, 4=LZ4
 [bytes × (byte_length-1): compressed_nbt_chunk_data]
 [padding: fill to next 4096-byte sector boundary]
```

### Diagram

```
Offset 0                 4096                8192
│     Location Table      │   Timestamp Table  │   Chunk Data ...
│ [loc0][loc1]...[loc1023] │ [ts0][ts1]..[ts1023]│ [chunk@sector N]...
│  4 bytes each            │  4 bytes each       │
```

---

## 7. BlockPos Packed i64

`BlockPos` values are packed into a single `i64` in the protocol and in NBT:

```
 Bit layout:
 63       46 45      26 25    0
 ┌──────────┬──────────┬────────┐
 │  X (26b) │  Z (26b) │ Y (12b)│
 └──────────┴──────────┴────────┘

 X: bits [63:38]  — signed 26-bit, range −33554432..33554431
 Z: bits [37:12]  — signed 26-bit, range −33554432..33554431
 Y: bits [11:0]   — signed 12-bit, range −2048..2047
```

### Encoding

```rust
pub fn pack_block_pos(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FFFFFF) << 38)
  | ((z as i64 & 0x3FFFFFF) << 12)
  | (y as i64 & 0xFFF)
}
```

### Decoding

```rust
pub fn unpack_block_pos(packed: i64) -> (i32, i32, i32) {
    let x = (packed >> 38) as i32;
    let z = (packed << 26 >> 38) as i32;  // sign-extend 26 bits
    let y = (packed << 52 >> 52) as i32;  // sign-extend 12 bits
    (x, y, z)
}
```

---

## 8. Entity Metadata Wire Format

Entity metadata is sent as a variable-length list of (index, type, value)
tuples, terminated by `0xFF`.

```
 ┌──────────┬───────────┬──────────────────────────┐
 │ index u8 │ type VarInt│ value bytes               │
 └──────────┴───────────┴──────────────────────────┘
 ...repeated...
 ┌──────────┐
 │  0xFF    │  ← end sentinel
 └──────────┘
```

### Metadata type IDs

| ID | Rust Type | Wire format |
|----|----------|-------------|
| 0 | `i8` | 1 byte |
| 1 | `VarInt` | VarInt |
| 2 | `VarLong` | VarLong |
| 3 | `f32` | 4 bytes IEEE 754 BE |
| 4 | `String` | VarInt len + UTF-8 bytes |
| 5 | `Component` (chat) | NBT tag |
| 6 | `Option<Component>` | bool + optional NBT |
| 7 | `ItemStack` | NBT (or `0x00` for empty) |
| 8 | `bool` | 1 byte (0 or 1) |
| 9 | `Vec3` (rotation) | 3 × f32 BE (pitch, yaw, roll) |
| 10 | `BlockPos` | packed i64 |
| 11 | `Option<BlockPos>` | bool + optional packed i64 |
| 12 | `Direction` | VarInt (0–5: down/up/north/south/west/east) |
| 13 | `Option<UUID>` | bool + optional 2×i64 |
| 14 | `BlockState` | VarInt (global block state ID) |
| 15 | `Option<BlockState>` | VarInt (0 = absent) |
| 16 | `NBT` | NBT tag |
| 17 | `Particle` | VarInt particle ID + params |
| 18 | `ParticleList` | VarInt count + Particle × count |
| 19 | `VillagerData` | 3 × VarInt (type, profession, level) |
| 20 | `Option<VarInt>` | VarInt (0 = absent; actual = value+1) |
| 21 | `EntityPose` | VarInt (0=standing, 1=fall_flying, 2=sleeping…) |
| 22 | `CatVariant` | VarInt |
| 23 | `WolfVariant` | VarInt |
| 24 | `FrogVariant` | VarInt |
| 25 | `Option<GlobalPos>` | bool + optional (ResourceKey + BlockPos) |
| 26 | `PaintingVariant` | VarInt |
| 27 | `SnifferState` | VarInt |
| 28 | `ArmadilloState` | VarInt |
| 29 | `Vec3` (3D vector) | 3 × f64 BE |
| 30 | `Quaternion` | 4 × f32 BE |

### Example — Entity flags (index 0, type 0)

```
00    ← index = 0
00    ← type = byte (0)
20    ← value = 0x20 (bit 5 = invisible)
FF    ← end
```

---

## 9. UUID Wire Format

UUIDs are sent as two consecutive big-endian `i64` values (MSB first, LSB second).
The UUID is split at the 64-bit boundary:

```
 UUID:  xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
 MSB:   bytes 0–7  (first 8 bytes = high 64 bits)
 LSB:   bytes 8–15 (last 8 bytes  = low 64 bits)

 Wire:
 ┌─────────────────┬─────────────────┐
 │  i64 BE  (MSB)  │  i64 BE  (LSB)  │
 │  8 bytes        │  8 bytes        │
 └─────────────────┴─────────────────┘
```

Example UUID `550e8400-e29b-41d4-a716-446655440000`:

```
MSB bytes: 55 0e 84 00 e2 9b 41 d4
LSB bytes: a7 16 44 66 55 44 00 00
```

```rust
pub fn encode_uuid(uuid: Uuid, buf: &mut impl BufMut) {
    let (msb, lsb) = uuid.as_u64_pair();
    buf.put_i64(msb as i64);
    buf.put_i64(lsb as i64);
}

pub fn decode_uuid(buf: &mut impl Buf) -> Uuid {
    let msb = buf.get_i64() as u64;
    let lsb = buf.get_i64() as u64;
    Uuid::from_u64_pair(msb, lsb)
}
```

---

## 10. Position Encoding: Angle Byte

Entity yaw and pitch are sent as a single `u8` representing a fraction of a
full rotation:

```
angle_byte = (degrees / 360.0 * 256.0) as u8
```

| Degrees | Byte |
|---------|------|
| 0° | `0x00` |
| 90° | `0x40` |
| 180° | `0x80` |
| 270° | `0xC0` |
| 360° | `0x00` (wraps) |

Precision: `360 / 256 ≈ 1.41°` per step.

---

## 11. Fixed-Point Delta Position

For `ClientboundMoveEntityPacket` (relative move), positions are encoded as
signed 16-bit fixed-point with 12 fractional bits:

```
delta = (new_pos * 4096) - (old_pos * 4096)
```

Must fit in `i16` (range ≈ ±8 blocks). If delta exceeds this range, a
`ClientboundTeleportEntityPacket` is sent instead.

```rust
pub fn encode_pos_delta(old: f64, new: f64) -> i16 {
    ((new * 4096.0) - (old * 4096.0)) as i16
}

pub fn decode_pos_delta(delta: i16, old: f64) -> f64 {
    old + delta as f64 / 4096.0
}
```
