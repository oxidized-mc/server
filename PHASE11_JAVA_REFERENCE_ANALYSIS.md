# Phase 11 Concepts - Java Reference Analysis

## 1. Block Update Flags Constants (from Block.java)

```java
public static final int UPDATE_NEIGHBORS = 1;          // 0x1
public static final int UPDATE_CLIENTS = 2;            // 0x2  
public static final int UPDATE_INVISIBLE = 4;          // 0x4
public static final int UPDATE_IMMEDIATE = 8;          // 0x8
public static final int UPDATE_KNOWN_SHAPE = 16;       // 0x10
public static final int UPDATE_SUPPRESS_DROPS = 32;    // 0x20
public static final int UPDATE_MOVE_BY_PISTON = 64;    // 0x40
public static final int UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE = 128;     // 0x80
public static final int UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS = 256; // 0x100
public static final int UPDATE_SKIP_ON_PLACE = 512;    // 0x200
public static final int UPDATE_LIMIT = 512;            // Default recursion limit
```

## 2. ServerLevel.java Analysis

### Fields and Constructor
- **chunkSource**: ServerChunkCache instance for chunk management
- **server**: Reference to MinecraftServer
- **serverLevelData**: ServerLevelData for level properties
- **entityManager**: PersistentEntitySectionManager for entity management
- **gameEventDispatcher**: Handles game events
- **blockTicks/fluidTicks**: Scheduled tick queues
- **dimension**: ResourceKey<Level> identifying the world (OVERWORLD, NETHER, END)

### Chunk Access Pattern
```java
public ServerChunkCache getChunkSource() {
    return this.chunkSource;
}

// Delegates to ChunkSource
public @Nullable ChunkAccess getChunk(int chunkX, int chunkZ, ChunkStatus status, boolean loadOrGenerate) {
    ChunkAccess chunk = this.getChunkSource().getChunk(chunkX, chunkZ, status, loadOrGenerate);
    if (chunk == null && loadOrGenerate) {
        throw new IllegalStateException("Should always be able to create a chunk!");
    }
    return chunk;
}

// Convenience methods
public LevelChunk getChunk(int chunkX, int chunkZ) {
    return (LevelChunk) this.getChunk(chunkX, chunkZ, ChunkStatus.FULL);
}

public LevelChunk getChunkAt(BlockPos pos) {
    return this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), 
                         SectionPos.blockToSectionCoord(pos.getZ()));
}
```

### Dirty Chunk Tracking
- Chunks are marked dirty via `markUnsaved()` in LevelChunk
- ServerChunkCache has a `blockChanged(BlockPos pos)` method that marks chunks dirty
- When block changes occur, ServerLevel calls `this.getChunkSource().blockChanged(pos)`
- The unsaved listener pattern allows tracking of which chunks need to be saved

## 3. Level.java Analysis

### Block State Management - setBlock() Flow
```java
public boolean setBlock(BlockPos pos, BlockState blockState, @Block.UpdateFlags int updateFlags, int updateLimit) {
    // 1. Validate bounds
    if (!this.isInValidBounds(pos)) return false;
    if (!this.isClientSide() && this.isDebug()) return false;
    
    // 2. Get chunk and delegate
    LevelChunk chunk = this.getChunkAt(pos);
    Block block = blockState.getBlock();
    BlockState oldState = chunk.setBlockState(pos, blockState, updateFlags);
    if (oldState == null) return false;
    
    // 3. Get new state and verify change
    BlockState newState = this.getBlockState(pos);
    if (newState == blockState) {
        // 4. Mark dirty if state changed
        if (oldState != newState) {
            this.setBlocksDirty(pos, oldState, newState);
        }
        
        // 5. Handle UPDATE_CLIENTS flag (send to players)
        if ((updateFlags & UPDATE_CLIENTS) != 0
            && (!this.isClientSide() || (updateFlags & UPDATE_INVISIBLE) == 0)
            && (this.isClientSide() || chunk.getFullStatus() != null 
                && chunk.getFullStatus().isOrAfter(FullChunkStatus.BLOCK_TICKING))) {
            this.sendBlockUpdated(pos, oldState, blockState, updateFlags);
        }
        
        // 6. Handle UPDATE_NEIGHBORS flag
        if ((updateFlags & UPDATE_NEIGHBORS) != 0) {
            this.updateNeighborsAt(pos, oldState.getBlock());
            if (!this.isClientSide() && blockState.hasAnalogOutputSignal()) {
                this.updateNeighbourForOutputSignal(pos, block);
            }
        }
        
        // 7. Handle neighbor shape updates (unless UPDATE_KNOWN_SHAPE set)
        if ((updateFlags & UPDATE_KNOWN_SHAPE) == 0 && updateLimit > 0) {
            int neighbourUpdateFlags = updateFlags & -34;  // Remove various flags
            oldState.updateIndirectNeighbourShapes(this, pos, neighbourUpdateFlags, updateLimit - 1);
            blockState.updateNeighbourShapes(this, pos, neighbourUpdateFlags, updateLimit - 1);
            blockState.updateIndirectNeighbourShapes(this, pos, neighbourUpdateFlags, updateLimit - 1);
        }
    }
    return true;
}
```

### getBlockState() Implementation
```java
public BlockState getBlockState(BlockPos pos) {
    if (!this.isInValidBounds(pos)) {
        return Blocks.VOID_AIR.defaultBlockState();
    } else {
        LevelChunk chunk = this.getChunk(SectionPos.blockToSectionCoord(pos.getX()), 
                                         SectionPos.blockToSectionCoord(pos.getZ()));
        return chunk.getBlockState(pos);
    }
}
```

### Key Constants
- `MAX_LEVEL_SIZE = 30000000` (world border)
- `LONG_PARTICLE_CLIP_RANGE = 512`
- `SHORT_PARTICLE_CLIP_RANGE = 32`
- Default UPDATE_LIMIT = 512

## 4. BlockGetter.java Interface

Complete read-only interface for block state access:
```java
public interface BlockGetter extends LevelHeightAccessor {
    @Nullable BlockEntity getBlockEntity(BlockPos pos);
    BlockState getBlockState(BlockPos pos);
    FluidState getFluidState(BlockPos pos);
    
    default int getLightEmission(BlockPos pos) {
        return this.getBlockState(pos).getLightEmission();
    }
    // ... raycast and voxel shape methods
}
```

## 5. LevelReader.java Key Methods

```java
public interface LevelReader extends BlockAndLightGetter, CollisionGetter, SignalGetter {
    @Nullable ChunkAccess getChunk(int chunkX, int chunkZ, ChunkStatus targetStatus, boolean loadOrGenerate);
    
    boolean hasChunk(int chunkX, int chunkZ);
    
    int getHeight(Heightmap.Types type, int x, int z);
    
    int getSkyDarken();
    
    BiomeManager getBiomeManager();
    
    DimensionType dimensionType();
    
    // Default implementations delegate to dimensionType()
    default int getMinY() {
        return this.dimensionType().minY();
    }
    
    default int getHeight() {
        return this.dimensionType().height();
    }
}
```

## 6. LevelAccessor.java Key Methods

Extends LevelReader with write capabilities:
```java
public interface LevelAccessor extends CommonLevelAccessor, ScheduledTickAccess {
    long nextSubTickCount();  // For tick scheduling
    
    LevelData getLevelData();
    
    default long getGameTime() {
        return this.getLevelData().getGameTime();
    }
    
    @Nullable MinecraftServer getServer();
    
    ChunkSource getChunkSource();
    
    default boolean hasChunk(int chunkX, int chunkZ) {
        return this.getChunkSource().hasChunk(chunkX, chunkZ);
    }
}
```

## 7. LevelWriter.java Interface

Pure writing interface:
```java
public interface LevelWriter {
    boolean setBlock(BlockPos pos, BlockState blockState, @Block.UpdateFlags int updateFlags, int updateLimit);
    
    default boolean setBlock(BlockPos pos, BlockState blockState, @Block.UpdateFlags int updateFlags) {
        return this.setBlock(pos, blockState, updateFlags, 512);  // Default limit
    }
    
    boolean removeBlock(BlockPos pos, boolean movedByPiston);
    
    boolean destroyBlock(BlockPos pos, boolean dropResources, @Nullable Entity breaker, int updateLimit);
}
```

## 8. ServerChunkCache.java Analysis

### Caching Strategy
- **4-entry LRU cache** using arrays (lastChunkPos, lastChunkStatus, lastChunk)
- Stores: chunk position (packed long), ChunkStatus, ChunkAccess reference
- `storeInCache()` shifts array entries left and inserts at index 0

```java
private static final int CACHE_SIZE = 4;
private final long[] lastChunkPos = new long[4];
private final @Nullable ChunkStatus[] lastChunkStatus = new ChunkStatus[4];
private final @Nullable ChunkAccess[] lastChunk = new ChunkAccess[4];

private void storeInCache(long pos, @Nullable ChunkAccess chunk, ChunkStatus status) {
    for (int i = 3; i > 0; i--) {
        this.lastChunkPos[i] = this.lastChunkPos[i - 1];
        this.lastChunkStatus[i] = this.lastChunkStatus[i - 1];
        this.lastChunk[i] = this.lastChunk[i - 1];
    }
    this.lastChunkPos[0] = pos;
    this.lastChunkStatus[0] = status;
    this.lastChunk[0] = chunk;
}
```

### getChunk() Method
```java
public @Nullable ChunkAccess getChunk(int x, int z, ChunkStatus targetStatus, boolean loadOrGenerate) {
    if (Thread.currentThread() != this.mainThread) {
        // Off-thread access: submit to main thread
        return CompletableFuture.<ChunkAccess>supplyAsync(
            () -> this.getChunk(x, z, targetStatus, loadOrGenerate), 
            this.mainThreadProcessor
        ).join();
    } else {
        ProfilerFiller profiler = Profiler.get();
        profiler.incrementCounter("getChunk");
        long pos = ChunkPos.pack(x, z);
        
        // Check 4-entry cache
        for (int i = 0; i < 4; i++) {
            if (pos == this.lastChunkPos[i] && targetStatus == this.lastChunkStatus[i]) {
                ChunkAccess chunkAccess = this.lastChunk[i];
                if (chunkAccess != null || !loadOrGenerate) {
                    return chunkAccess;  // Cache hit
                }
            }
        }
        
        profiler.incrementCounter("getChunkCacheMiss");
        // Fetch from ChunkMap
        CompletableFuture<ChunkResult<ChunkAccess>> serverFuture = 
            this.getChunkFutureMainThread(x, z, targetStatus, loadOrGenerate);
        this.mainThreadProcessor.managedBlock(serverFuture::isDone);
        ChunkResult<ChunkAccess> chunkResult = serverFuture.join();
        ChunkAccess chunk = chunkResult.orElse(null);
        if (chunk == null && loadOrGenerate) {
            throw Util.pauseInIde(new IllegalStateException("Chunk not there when requested: " + chunkResult.getError()));
        }
        this.storeInCache(pos, chunk, targetStatus);
        return chunk;
    }
}
```

### ChunkMap and Visibility
- `getVisibleChunkIfPresent(long key)`: Returns ChunkHolder if chunk is currently loaded
- ChunkMap manages distance tickets and chunk loading/unloading
- Distance manager controls which chunks are loaded based on player proximity

## 9. DimensionType.java Analysis

### Record Fields
```java
record DimensionType(
    boolean hasFixedTime,
    boolean hasSkyLight,
    boolean hasCeiling,
    boolean hasEnderDragonFight,
    double coordinateScale,
    int minY,              // Height minimum (must be multiple of 16)
    int height,            // Total height (must be multiple of 16, >= 16)
    int logicalHeight,     // Server-side render height (can be < height)
    TagKey<Block> infiniburn,
    float ambientLight,
    MonsterSettings monsterSettings,
    Skybox skybox,
    CardinalLighting.Type cardinalLightType,
    EnvironmentAttributeMap attributes,
    HolderSet<Timeline> timelines,
    Optional<Holder<WorldClock>> defaultClock
)
```

### Height Constants
```java
public static final int BITS_FOR_Y = BlockPos.PACKED_Y_LENGTH;  // 10 bits
public static final int MIN_HEIGHT = 16;
public static final int Y_SIZE = (1 << BITS_FOR_Y) - 32;        // 992 blocks
public static final int MAX_Y = (Y_SIZE >> 1) - 1;              // 495
public static final int MIN_Y = MAX_Y - Y_SIZE + 1;             // -496
public static final int WAY_ABOVE_MAX_Y = MAX_Y << 4;
public static final int WAY_BELOW_MIN_Y = MIN_Y << 4;
```

### Constructor Validation
```java
public DimensionType(...) {
    if (height < 16) 
        throw new IllegalStateException("height has to be at least 16");
    if (minY + height > MAX_Y + 1)
        throw new IllegalStateException("min_y + height cannot be higher than: " + (MAX_Y + 1));
    if (logicalHeight > height)
        throw new IllegalStateException("logical_height cannot be higher than height");
    if (height % 16 != 0)
        throw new IllegalStateException("height has to be multiple of 16");
    if (minY % 16 != 0)
        throw new IllegalStateException("min_y has to be a multiple of 16");
    // ... store fields
}
```

## 10. LevelChunk.java Key Methods

### setBlockState() Implementation
```java
public @Nullable BlockState setBlockState(BlockPos pos, BlockState state, @Block.UpdateFlags int flags) {
    int sectionY = this.getSectionIndex(pos.getY());
    if (sectionY < 0 || sectionY >= this.sections.length) {
        return null;
    }
    
    LevelChunkSection section = this.sections[sectionY];
    int localX = pos.getX() & 15;
    int localY = pos.getY() & 15;
    int localZ = pos.getZ() & 15;
    
    BlockState oldState = section.setBlockState(localX, localY, localZ, state);
    if (oldState == state) return oldState;
    
    Block oldBlock = oldState.getBlock();
    Block newBlock = state.getBlock();
    
    if (!oldBlock.equals(newBlock)) {
        // Update heightmaps
        this.heightmaps.forEach((type, heightmap) -> heightmap.update(pos.getX(), pos.getY(), pos.getZ()));
        
        // Handle block entities
        if (oldBlock.hasBlockEntity()) {
            this.removeBlockEntity(pos);
        }
        if (newBlock.hasBlockEntity() && (flags & UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS) == 0) {
            // Handle block entity placement
        }
    }
    
    this.markUnsaved();
    return oldState;
}

public BlockState getBlockState(BlockPos pos) {
    int sectionY = this.getSectionIndex(pos.getY());
    if (sectionY < 0 || sectionY >= this.sections.length) {
        return Blocks.VOID_AIR.defaultBlockState();
    }
    LevelChunkSection section = this.sections[sectionY];
    return section.getBlockState(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15);
}
```

### Dirty Tracking
```java
public void markUnsaved() {
    boolean wasUnsaved = this.isUnsaved();
    super.markUnsaved();
    if (!wasUnsaved) {
        this.unsavedListener.setUnsaved(this.chunkPos);
    }
}
```

### Full Status
```java
public FullChunkStatus getFullStatus() {
    // Returns: INACCESSIBLE, FULL, BLOCK_TICKING, or ENTITY_TICKING
}
```

## 11. ChunkStatus Hierarchy

```
EMPTY (protochunk) 
  ↓ STRUCTURE_STARTS
  ↓ STRUCTURE_REFERENCES
  ↓ BIOMES
  ↓ NOISE
  ↓ SURFACE
  ↓ CARVERS
  ↓ FEATURES
  ↓ INITIALIZE_LIGHT
  ↓ LIGHT
  ↓ SPAWN
  ↓ FULL (LevelChunk, ready for gameplay)
```

### Checking Status
```java
public boolean isOrAfter(ChunkStatus step) {
    return this.getIndex() >= step.getIndex();
}

public boolean isOrBefore(ChunkStatus step) {
    return this.getIndex() <= step.getIndex();
}
```

## Key Patterns and Algorithms

### 1. Block Access Pattern
1. Convert BlockPos to chunk coordinates: `SectionPos.blockToSectionCoord(pos.getX/Z)`
2. Get chunk from ChunkSource with appropriate ChunkStatus
3. Delegate to chunk's getBlockState/setBlockState
4. Chunk extracts section: `sectionIndex = getSectionIndex(y)`
5. Section extracts block from palette: `section.getBlockState(x & 15, y & 15, z & 15)`

### 2. Block Update Pattern
1. Call `setBlock(pos, newState, flags)`
2. Get old state from chunk
3. If successful, check UPDATE_CLIENTS flag → send packet if set
4. Check UPDATE_NEIGHBORS flag → notify adjacent blocks
5. Check UPDATE_KNOWN_SHAPE flag → if not set, recursively update neighbor shapes
6. Mark chunk dirty via `markUnsaved()`

### 3. Chunk Loading Strategy
- **Main thread only**: Chunk operations must happen on the server's main thread
- **Cache-first**: 4-entry LRU cache in ServerChunkCache
- **Distance-based**: ChunkMap uses distance manager to load/unload chunks based on view distance
- **Status-dependent**: Chunks progress through 12 statuses from EMPTY to FULL

### 4. Bounds Checking Pattern
```java
isInValidBounds(pos): Check if pos is within ChunkPos valid range
isInWorldBounds(pos): Check if pos is within 30M horizontal + height bounds
isInsideBuildHeight(pos): Check if y is within dimension's minY/maxY
```

### 5. Neighbor Update Pattern
- Called when block state changes
- Recursively propagates updates to adjacent blocks
- UPDATE_LIMIT (512) prevents infinite recursion
- updateNeighbourShapes() handles voxel shape changes

## Missing Patterns in Rust Implementation

Based on the Java reference, here are likely missing or incomplete patterns:

1. **4-entry LRU chunk cache** - ServerChunkCache caches last 4 chunks accessed
2. **Thread safety** - Java version checks `Thread.currentThread() != this.mainThread` and submits off-thread accesses
3. **Flag compositing** - Complex bitwise flag operations with UPDATE_SKIP_* flags
4. **Recursive shape updates** - updateNeighbourShapes/updateIndirectNeighbourShapes with updateLimit
5. **FullChunkStatus checks** - Blocks only send updates if chunk is at least BLOCK_TICKING status
6. **DimensionType validation** - Height must be multiple of 16, minY % 16 == 0, etc.
7. **Heightmap updates** - Chunks maintain multiple heightmaps (OCEAN_FLOOR, WORLD_SURFACE, etc.)
8. **Block entity handling** - Special logic for blocks with entities, BlockState setBlockState() calls
9. **Dirty chunk tracking** - Chunks marked unsaved in markUnsaved(), notifies listeners
10. **ChunkStatus hierarchy** - 12-status progression, not just FULL/not-full
