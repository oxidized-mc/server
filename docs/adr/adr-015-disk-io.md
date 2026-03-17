# ADR-015: Disk I/O & Persistence Strategy

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P10, P20 |
| Deciders | Oxidized Core Team |

## Context

A Minecraft server must persist world data to disk reliably. Chunk data is stored in Anvil region
files (`.mca`), where each region file covers a 32×32 chunk area. Player data is stored as
individual NBT `.dat` files. The `level.dat` file stores world metadata (seed, game rules, time,
weather). All of these must be read at startup (or on demand for chunk loading) and written
periodically during gameplay and at shutdown.

Vanilla Java uses blocking I/O on a dedicated "I/O thread" for chunk saves, with the main game
thread blocking on chunk loads (reads). Region files use a sector-based format: an 8 KB header
(4 KB offset table + 4 KB timestamp table) followed by variable-size sectors (4 KB each). Each
chunk's data is compressed (zlib by default) and stored in one or more contiguous sectors. The
offset table maps chunk positions to sector offsets within the file. This format is compact and
supports random access to individual chunks, but the variable-size sectors and in-place updates
create complexity around fragmentation and crash safety.

For Oxidized, we need a persistence strategy that never blocks the game tick loop, handles
crash recovery gracefully (a hard crash should not corrupt the world), and achieves throughput
sufficient for autosaving thousands of chunks every 5 minutes. We must also maintain full
compatibility with the Anvil region file format so that existing vanilla worlds can be loaded
and saved without conversion.

## Decision Drivers

- **Non-blocking tick loop**: Disk I/O must never stall the game tick. Reads and writes happen
  off the main tick thread.
- **Crash safety**: A server crash (power loss, OOM kill, panic) should not corrupt the world.
  At worst, a few seconds of changes are lost; the world should always be loadable.
- **Throughput**: Autosave must flush thousands of dirty chunks within a reasonable time window
  (< 30 seconds for 10,000 chunks) without causing lag spikes.
- **Anvil compatibility**: We must read and write the vanilla Anvil region file format (`.mca`)
  so that existing worlds work without conversion.
- **Memory efficiency**: Region file headers should be cached to avoid repeated header reads, but
  we should not memory-map entire region files (see rationale below).
- **Compression performance**: Chunk data compression/decompression is CPU-bound; we should
  choose a compression level that balances speed and ratio.

## Considered Options

### Option 1: tokio::task::spawn_blocking With Standard File I/O

Offload all file operations to tokio's blocking thread pool via `spawn_blocking`. Each read or
write operation gets a dedicated OS thread from the pool. Reads use `std::fs::File` with `seek`
and `read_exact`; writes use temporary files and atomic renames where possible. This is simple,
well-understood, and leverages Rust's standard library. The blocking thread pool automatically
scales to handle concurrent operations. The downside is that each operation may block an OS
thread, limiting scalability for very high I/O rates, but for Minecraft's workload (hundreds of
chunks per second, not thousands), this is more than sufficient.

### Option 2: Memory-Mapped Files (mmap)

Map each region file into virtual memory and access chunk data via pointer arithmetic. Reads
become memory accesses, and the kernel handles page faults and caching. However, Anvil's
variable-size sectors do not map well to mmap — when a chunk grows (e.g. player builds a
complex structure, increasing NBT size), it may need to be relocated to new sectors, requiring
file resizing and remap. Additionally, mmap does not help with the zlib decompression step, which
is the actual bottleneck. Finally, mmap introduces platform-specific behavior for I/O errors
(SIGBUS on Linux) that is difficult to handle safely in Rust. The theoretical benefits
(zero-copy reads) are negated by the practical realities of the Anvil format.

### Option 3: io_uring via tokio-uring

Use Linux's io_uring interface for truly asynchronous file I/O without blocking threads. This
provides the highest possible I/O throughput and efficiency. However, `tokio-uring` is not yet
stable, io_uring is Linux-only (no macOS, no Windows), and the complexity of managing io_uring
submission and completion queues adds significant implementation burden. For a Minecraft server's
I/O workload (modest by database standards), the performance benefit over `spawn_blocking` is
negligible.

### Option 4: Dedicated I/O Thread With Channel-Based Requests

A single dedicated thread handles all file I/O, receiving read/write requests via a channel.
This serializes all I/O through one thread, avoiding concurrent file access issues. However,
serializing I/O limits throughput — chunk loads during player teleportation benefit from parallel
reads. A single I/O thread cannot saturate modern SSDs. This approach would need multiple I/O
threads to achieve adequate throughput, at which point it is effectively a custom thread pool —
reinventing `spawn_blocking`.

### Option 5: Write-Ahead Log (WAL) + Periodic Flush

Write all chunk modifications to a sequential WAL file first (fast, sequential I/O), then
periodically apply the WAL to the region files. This maximizes crash safety (WAL is always
consistent) and write throughput (sequential writes). However, reading a chunk now requires
checking the WAL before the region file, adding complexity. The WAL must also be garbage
collected after flush. For a Minecraft server that autosaves every 5 minutes, the WAL would
accumulate significant data. The added complexity is not justified given that sector-level writes
to region files are already reasonably crash-safe.

## Decision

We adopt **spawn_blocking for file I/O + write coalescing + dirty chunk queue**. This provides
non-blocking I/O with a simple, well-understood implementation.

### Read Path

When a chunk is requested (player enters view distance, worldgen needs neighbor):

```rust
pub async fn load_chunk(&self, pos: ChunkPos) -> Result<Option<ChunkColumn>> {
    let region_pos = pos.region_pos();
    let region = self.get_or_open_region(region_pos).await?;

    tokio::task::spawn_blocking(move || {
        let mut file = region.file.lock();
        let (offset, size) = region.header.chunk_location(pos)?;
        if offset == 0 { return Ok(None); } // chunk not saved yet

        file.seek(SeekFrom::Start(offset as u64 * 4096))?;
        let mut sector_data = vec![0u8; size as usize * 4096];
        file.read_exact(&mut sector_data)?;

        let length = u32::from_be_bytes(sector_data[0..4].try_into()?) as usize;
        let compression = sector_data[4];
        let compressed = &sector_data[5..5 + length - 1];

        let decompressed = decompress(compression, compressed)?;
        let nbt = nbt::from_bytes(&decompressed)?;
        Ok(Some(ChunkColumn::from_nbt(pos, nbt)?))
    }).await?
}
```

Region file headers are cached in memory to avoid re-reading the 8 KB header on every chunk
access. A `RegionCache` holds recently accessed region file handles:

```rust
pub struct RegionCache {
    regions: Mutex<LruCache<RegionPos, Arc<RegionFile>>>,
    max_open_files: usize, // default: 256
}

pub struct RegionFile {
    file: Mutex<File>,
    header: RegionHeader, // 4KB offsets + 4KB timestamps, parsed once
}
```

### Write Path (Coalesced Saves)

Rather than writing each dirty chunk immediately, writes are coalesced through a dirty queue:

```rust
pub struct SaveScheduler {
    dirty_chunks: DashMap<ChunkPos, Arc<ChunkColumn>>,
    autosave_interval: u64, // ticks, default: 6000 (5 minutes)
}
```

When a chunk is modified (block placement, entity change, lighting update), it is marked dirty
(`chunk.dirty.store(true, Ordering::Relaxed)`) and added to the dirty map. A periodic save task
runs every `autosave_interval` ticks:

```rust
async fn autosave(&self) {
    let dirty: Vec<_> = self.dirty_chunks.iter()
        .map(|entry| (*entry.key(), Arc::clone(entry.value())))
        .collect();

    for batch in dirty.chunks(BATCH_SIZE) {
        let mut by_region: HashMap<RegionPos, Vec<_>> = HashMap::new();
        for (pos, chunk) in batch {
            by_region.entry(pos.region_pos()).or_default().push((pos, chunk));
        }

        for (region_pos, chunks) in by_region {
            let region = self.get_or_open_region(region_pos).await?;
            tokio::task::spawn_blocking(move || {
                let mut file = region.file.lock();
                for (pos, chunk) in chunks {
                    let nbt = chunk.to_nbt();
                    let compressed = compress(nbt, COMPRESSION_LEVEL)?;
                    write_chunk_to_region(&mut file, &region.header, pos, &compressed)?;
                    chunk.dirty.store(false, Ordering::Relaxed);
                }
            }).await?;
        }
    }
}
```

Chunks destined for the same region file are batched together, minimizing file open/seek
overhead. The save task runs on a background tokio task, never blocking the game tick.

### Compression

Vanilla uses zlib compression at level 6 for chunk data. We use **zlib level 4** as the default,
which provides approximately 90% of the compression ratio at roughly 60% of the CPU time (based
on benchmarks with typical chunk data). The compression format byte in the sector header is
preserved for compatibility:

| Value | Format |
|-------|--------|
| 1 | gzip (legacy, read-only support) |
| 2 | zlib (default for writing) |
| 3 | uncompressed (supported for debugging) |
| 4 | lz4 (future option, not yet used) |

All formats are supported for reading; new writes always use zlib (format 2) for maximum
compatibility with vanilla and third-party tools.

### Crash Safety

Region file writes at the sector level are the smallest atomic unit. If a crash occurs mid-write:

- **Partially written sector**: The chunk's data may be corrupted, but the region header still
  points to the old sectors (we update the header after writing data). On next load, the chunk
  is either the old version or fails to decompress (detected by zlib CRC), in which case we log
  a warning and treat it as ungenerated.
- **Header update**: The header is written after data sectors. If the crash occurs between data
  write and header update, the old header still points to the old data — no corruption.
- **level.dat**: Written via atomic rename (`write to level.dat_new`, then `rename` to
  `level.dat`). A `level.dat_old` backup is maintained. If `level.dat` is corrupt on startup,
  fall back to `level.dat_old`.

Write ordering is: data sectors first → header update → fsync. This ensures that even without
fsync (which we call once per autosave batch, not per chunk), the worst case is reverting to the
previous version of a chunk.

### Player Data

Player data (`.dat` files in `world/playerdata/`) is saved:
- On player disconnect (immediate async save)
- Every 5 minutes for connected players (batched with autosave)

Each player file is written via atomic rename for crash safety:

```rust
async fn save_player(&self, uuid: Uuid, data: &PlayerData) -> Result<()> {
    let path = self.player_dir.join(format!("{uuid}.dat"));
    let temp = self.player_dir.join(format!("{uuid}.dat.tmp"));

    tokio::task::spawn_blocking(move || {
        let nbt = data.to_nbt();
        let compressed = gzip_compress(&nbt)?;
        std::fs::write(&temp, compressed)?;
        std::fs::rename(&temp, &path)?;
        Ok(())
    }).await?
}
```

### Why Not mmap

Memory-mapped I/O seems attractive for region files but is poorly suited for the Anvil format:

1. **Variable-size sectors**: When a chunk grows, its sectors must be relocated. With mmap, this
   requires `mremap` or mapping a new range, both of which are complex and platform-specific.
2. **Compression**: Chunk data is zlib-compressed. mmap gives us the compressed bytes; we still
   need to decompress into a separate buffer. The "zero-copy" benefit is illusory.
3. **Error handling**: I/O errors during mmap access manifest as SIGBUS (Linux) or SEH exceptions
   (Windows), not as `Result::Err`. Handling these safely in Rust requires `unsafe` and
   platform-specific signal handlers.
4. **Write ordering**: mmap writes are flushed by the kernel at unpredictable times, making it
   difficult to enforce our "data before header" ordering for crash safety.

Standard file I/O with explicit seek/read/write gives us full control over ordering, error
handling, and buffering.

## Consequences

### Positive

- **Non-blocking game loop**: All disk I/O runs on tokio's blocking thread pool, never stalling
  the tick loop. Players experience no lag from chunk saves.
- **Crash safety**: Data-before-header write ordering, atomic rename for level.dat and player
  data, and fsync at batch boundaries provide strong crash recovery guarantees.
- **Throughput**: Write coalescing batches many dirty chunks into sequential writes to the same
  region file, maximizing disk throughput and minimizing seek overhead (on spinning disks) or
  write amplification (on SSDs).
- **Anvil compatibility**: Reading and writing the standard Anvil format ensures that existing
  vanilla worlds work without conversion, and third-party tools (MCEdit, Amulet) remain
  compatible.
- **Simplicity**: `spawn_blocking` with standard file I/O is straightforward to implement,
  debug, and maintain. No platform-specific APIs, no unsafe code.

### Negative

- **Thread pool sizing**: `spawn_blocking` uses tokio's default blocking thread pool (up to 512
  threads). Under extreme I/O load (many simultaneous chunk loads during teleportation), this
  could exhaust the pool. Mitigation: cap concurrent I/O operations with a semaphore.
- **No true async I/O**: `spawn_blocking` wraps synchronous I/O in a thread, not true async.
  Each in-flight I/O operation consumes an OS thread. For Minecraft's workload this is fine, but
  it is less efficient than io_uring for I/O-heavy scenarios.
- **Compression CPU cost**: zlib decompression on the blocking thread pool competes with other
  blocking tasks for CPU time. If compression becomes a bottleneck, a dedicated compression
  thread pool could be introduced.

### Neutral

- **Region file header caching**: Caching headers in memory uses ~8 KB per open region file. With
  256 open files, that is ~2 MB — negligible.
- **zlib level 4 vs 6**: Slightly larger files (~5% less compression) but measurably faster saves.
  Disk space is rarely a concern for Minecraft servers.

## Compliance

- **Round-trip test**: Generate a world with vanilla, load all chunks with Oxidized, save, reload
  with vanilla — verify no data loss or corruption.
- **Crash safety test**: Simulate crash (kill process) during autosave, restart, verify world is
  loadable and at most one autosave interval of data is lost.
- **Throughput benchmark**: Measure time to save 10,000 dirty chunks. Target: < 30 seconds on
  SSD, < 60 seconds on HDD.
- **Compression round-trip**: Verify that all compression formats (gzip, zlib, uncompressed) can
  be read and that written chunks use the correct format byte.
- **Player data test**: Save player data, kill process, restart, verify player data is intact.
- **Region cache test**: Open 300+ region files with cache size 256, verify LRU eviction works
  and no file descriptor leak.

## Related ADRs

- **ADR-010** (NBT Library): Chunk data is serialized as NBT before compression. The NBT
  library's serialization performance directly affects save throughput.
- **ADR-014** (Chunk Storage): The chunk storage layer marks chunks dirty; the disk I/O layer
  reads the dirty flag and flushes data. Chunk lifecycle (unload) requires a save-before-evict
  handshake.
- **ADR-016** (Worldgen Pipeline): Newly generated chunks are written to disk on their first
  autosave cycle. Worldgen may also need to load neighboring chunks from disk.
- **ADR-013** (Coordinate Types): `ChunkPos` and `RegionPos` coordinate conversions are used
  to map chunk positions to region file paths.

## References

- [Minecraft Wiki — Anvil File Format](https://minecraft.wiki/w/Anvil_file_format)
- [Minecraft Wiki — Region File Format](https://minecraft.wiki/w/Region_file_format)
- [tokio::task::spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
- [flate2 crate](https://docs.rs/flate2) — zlib compression for Rust
- [lru crate](https://docs.rs/lru) — LRU cache for region file handles
