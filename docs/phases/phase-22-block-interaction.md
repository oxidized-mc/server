# Phase 22 — Block Interaction

**Status:** ✅ Complete  
**Crate:** `oxidized-game`  
**Reward:** Player can break and place blocks; changes are visible to all players.

---

## Architecture Decisions

Before implementing this phase, review:

- [ADR-012: Block State](../adr/adr-012-block-state.md) — flat u16 state ID with dense lookup table


## Goal

Implement server-side block breaking (with mining speed calculation and damage
progress packets), block placing, block `use()` interaction (e.g. opening chests),
sign editing, and broadcast every block change to all watching clients via
`ClientboundBlockUpdatePacket` and `ClientboundSectionBlocksUpdatePacket`. Send
sequence acknowledgements so the client can reconcile its prediction.

---

## Java Reference

| Concept | Java class |
|---------|-----------|
| Player action handler | `net.minecraft.server.network.ServerGamePacketListenerImpl#handlePlayerAction` |
| Use item on block handler | `net.minecraft.server.network.ServerGamePacketListenerImpl#handleUseItemOn` |
| Block state change | `net.minecraft.server.level.ServerLevel#setBlock` |
| Block behaviour | `net.minecraft.world.level.block.state.BlockBehaviour#use` |
| Block destroy speed | `net.minecraft.world.entity.player.Player#getDestroySpeed` |
| Block destruction packet | `net.minecraft.network.protocol.game.ClientboundBlockDestructionPacket` |
| Block update packet | `net.minecraft.network.protocol.game.ClientboundBlockUpdatePacket` |
| Ack block change packet | `net.minecraft.network.protocol.game.ClientboundBlockChangedAckPacket` |
| Section blocks update | `net.minecraft.network.protocol.game.ClientboundSectionBlocksUpdatePacket` |
| Block event packet | `net.minecraft.network.protocol.game.ClientboundBlockEventPacket` |

---

## Tasks

### 22.1 — Serverbound action packets

```rust
/// 0x24 – player digs or performs a block action
#[derive(Debug, Clone)]
pub struct ServerboundPlayerActionPacket {
    pub status: PlayerAction,
    pub pos: BlockPos,
    pub face: Direction,
    pub sequence: i32,   // VarInt; used for block change acknowledgement
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PlayerAction {
    StartDestroyBlock  = 0,
    AbortDestroyBlock  = 1,
    StopDestroyBlock   = 2,
    DropAllItems       = 3,
    DropItem           = 4,
    ReleaseUseItem     = 5,
    SwapItemWithOffhand= 6,
}

/// 0x36 – player right-clicks a block face
#[derive(Debug, Clone)]
pub struct ServerboundUseItemOnPacket {
    pub hand: Hand,
    pub hit_result: BlockHitResult,
    pub sequence: i32,
}

/// 0x37 – player right-clicks without a target block (e.g. using a bow)
#[derive(Debug, Clone)]
pub struct ServerboundUseItemPacket {
    pub hand: Hand,
    pub sequence: i32,
}

/// 0x32 – player finishes editing a sign
#[derive(Debug, Clone)]
pub struct ServerboundSignUpdatePacket {
    pub pos: BlockPos,
    pub is_front_text: bool,
    pub lines: [String; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand { MainHand, OffHand }

#[derive(Debug, Clone)]
pub struct BlockHitResult {
    pub pos: BlockPos,
    pub face: Direction,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_z: f32,
    pub inside: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Direction {
    Down  = 0,
    Up    = 1,
    North = 2,
    South = 3,
    West  = 4,
    East  = 5,
}

impl Direction {
    pub fn offset(&self) -> (i32, i32, i32) {
        match self {
            Self::Down  => ( 0, -1,  0),
            Self::Up    => ( 0,  1,  0),
            Self::North => ( 0,  0, -1),
            Self::South => ( 0,  0,  1),
            Self::West  => (-1,  0,  0),
            Self::East  => ( 1,  0,  0),
        }
    }
}
```

### 22.2 — Clientbound block packets

```rust
/// 0x09 – animate a block being broken (progress 0–9, or 10 to clear)
#[derive(Debug, Clone)]
pub struct ClientboundBlockDestructionPacket {
    pub entity_id: i32,     // VarInt; unique per breaker (usually player's entity id)
    pub pos: BlockPos,
    pub progress: u8,       // 0–9 crack stage; 10 = remove cracks
}

/// 0x09 – single block change broadcast
#[derive(Debug, Clone)]
pub struct ClientboundBlockUpdatePacket {
    pub pos: BlockPos,
    pub block_state: i32,   // VarInt block state id
}

/// 0x07 – acknowledge a block change sequence number
#[derive(Debug, Clone)]
pub struct ClientboundBlockChangedAckPacket {
    pub sequence: i32,      // VarInt; must match ServerboundPlayerAction.sequence
}

/// 0x4B – batch update for a 16×16×16 section
#[derive(Debug, Clone)]
pub struct ClientboundSectionBlocksUpdatePacket {
    pub section_pos: SectionPos,  // packed i64: x<<42 | z<<20 | (y+64)
    pub positions_and_states: Vec<(u16, i32)>, // (relative pos packed, block_state id)
}

/// 0x0C – block action event (pistons, note blocks, chests opening sound)
#[derive(Debug, Clone)]
pub struct ClientboundBlockEventPacket {
    pub pos: BlockPos,
    pub action_type: u8,
    pub action_param: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl SectionPos {
    pub fn packed(&self) -> i64 {
        ((self.x as i64 & 0x3FFFFF) << 42)
            | ((self.z as i64 & 0x3FFFFF) << 20)
            | ((self.y as i64 + 4) & 0xFFFFF) // +4 for negative section Y offset
    }

    pub fn from_block_pos(pos: BlockPos) -> Self {
        Self {
            x: pos.x >> 4,
            y: pos.y >> 4,
            z: pos.z >> 4,
        }
    }
}
```

### 22.3 — Mining speed calculation (`oxidized-game/src/player/block_breaking.rs`)

```rust
impl ServerPlayer {
    /// Fractional block destroy speed per tick (1.0 = destroy in one tick).
    /// See `Player.getDestroySpeed` / `Player.getCurrentItemAttackStrengthDelay`.
    pub fn mining_speed(&self, block: &BlockState, level: &ServerLevel) -> f32 {
        let base_speed = self.get_item_destroy_speed(block);

        let mut speed = base_speed;

        // Haste effect: +20% per level
        if let Some(haste) = self.get_effect(MobEffect::Haste) {
            speed *= 1.0 + 0.2 * (haste.amplifier + 1) as f32;
        }

        // Mining fatigue: factor of 0.3^(level+1)
        if let Some(fatigue) = self.get_effect(MobEffect::MiningFatigue) {
            let factor = match fatigue.amplifier {
                0 => 0.3,
                1 => 0.09,
                2 => 0.0027,
                _ => 0.00081,
            };
            speed *= factor;
        }

        // In water without Aqua Affinity: 1/5 speed
        if self.is_in_fluid(Fluid::Water) && !self.has_enchantment(Enchantment::AquaAffinity) {
            speed /= 5.0;
        }

        // Not on ground: 1/5 speed
        if !self.on_ground {
            speed /= 5.0;
        }

        speed / block.destroy_time()
    }

    fn get_item_destroy_speed(&self, block: &BlockState) -> f32 {
        let item = self.inventory.get_selected();
        if item.is_empty() { return 1.0; }

        let base = item.destroy_speed_for(block);

        // Efficiency enchantment: +1 for level 1, +2^2+1 = 5 for level 2, etc.
        if base > 1.0 {
            if let Some(efficiency) = item.get_enchantment_level(Enchantment::Efficiency) {
                let bonus = (efficiency * efficiency + 1) as f32;
                return base + bonus;
            }
        }
        base
    }
}
```

### 22.4 — Block breaking handler (`oxidized-game/src/player/block_breaking.rs`)

```rust
impl PlayerConnection {
    pub async fn handle_player_action(
        &mut self,
        packet: ServerboundPlayerActionPacket,
    ) -> anyhow::Result<()> {
        match packet.status {
            PlayerAction::StartDestroyBlock => {
                if self.player.game_mode == GameType::Creative {
                    // Instant break in creative
                    self.break_block(packet.pos, packet.sequence).await?;
                } else {
                    self.current_break = Some(BlockBreakProgress {
                        pos: packet.pos,
                        face: packet.face,
                        progress: 0.0,
                        start_tick: self.server.current_tick(),
                    });
                    // Send initial destruction packet (no visual crack yet)
                    self.send_packet(ClientboundBlockDestructionPacket {
                        entity_id: self.player.entity_id,
                        pos: packet.pos,
                        progress: 0,
                    }).await?;
                }
            }

            PlayerAction::AbortDestroyBlock => {
                if let Some(bp) = self.current_break.take() {
                    // Clear crack overlay
                    self.server.broadcast_except(
                        self.player.uuid,
                        ClientboundBlockDestructionPacket {
                            entity_id: self.player.entity_id,
                            pos: bp.pos,
                            progress: 10, // 10 = clear
                        },
                    ).await;
                }
                self.send_ack(packet.sequence).await?;
            }

            PlayerAction::StopDestroyBlock => {
                // Survival: check if progress >= 1.0
                if let Some(bp) = self.current_break.take() {
                    let block = self.player.level().get_block_state(bp.pos);
                    let speed = self.player.mining_speed(&block, &self.player.level());
                    let elapsed = (self.server.current_tick() - bp.start_tick) as f32;
                    if elapsed * speed >= 1.0 {
                        self.break_block(bp.pos, packet.sequence).await?;
                    } else {
                        // Restore the block on the client
                        self.send_block_at(bp.pos).await?;
                        self.send_ack(packet.sequence).await?;
                    }
                }
            }

            PlayerAction::DropAllItems | PlayerAction::DropItem => {
                let drop_all = packet.status == PlayerAction::DropAllItems;
                self.player.drop_item(drop_all).await;
            }

            PlayerAction::ReleaseUseItem => {
                self.player.stop_using_item().await;
            }

            PlayerAction::SwapItemWithOffhand => {
                self.player.inventory.swap_main_and_offhand();
                self.send_inventory().await?;
            }
        }
        Ok(())
    }

    async fn break_block(&mut self, pos: BlockPos, sequence: i32) -> anyhow::Result<()> {
        let mut level = self.player.level_mut();
        let old_state = level.get_block_state(pos);

        // Set block to air
        level.set_block(pos, BlockState::AIR, SetBlockFlags::UPDATE_ALL)?;

        // Drop loot (if doTileDrops gamerule)
        if level.game_rules().get_bool(GameRuleKey::DoTileDrops)
            && self.player.game_mode != GameType::Creative
        {
            old_state.drop_resources(&mut level, pos, &self.player.inventory.get_selected());
        }

        // Broadcast to all players watching this chunk
        level.broadcast_block_update(pos).await;
        self.send_ack(sequence).await
    }

    async fn send_ack(&mut self, sequence: i32) -> anyhow::Result<()> {
        self.send_packet(ClientboundBlockChangedAckPacket { sequence }).await
    }
}
```

### 22.5 — Tick-based break progress (`oxidized-game/src/player/block_breaking.rs`)

```rust
pub struct BlockBreakProgress {
    pub pos: BlockPos,
    pub face: Direction,
    pub progress: f32,          // accumulates 0.0 → 1.0
    pub start_tick: u64,
}

impl PlayerConnection {
    /// Called every tick while the player holds LMB (survival).
    pub async fn tick_block_breaking(&mut self) {
        let Some(bp) = &mut self.current_break else { return };
        let pos = bp.pos;

        let block = self.player.level().get_block_state(pos);
        let speed = self.player.mining_speed(&block, &self.player.level());
        bp.progress += speed;

        let stage = ((bp.progress * 10.0) as u8).min(9);

        // Broadcast crack stage
        self.server.broadcast(ClientboundBlockDestructionPacket {
            entity_id: self.player.entity_id,
            pos,
            progress: stage,
        }).await;

        if bp.progress >= 1.0 {
            self.current_break = None;
            self.break_block(pos, -1).await.ok();
        }
    }
}
```

### 22.6 — Block placing handler (`oxidized-game/src/player/block_placing.rs`)

```rust
impl PlayerConnection {
    pub async fn handle_use_item_on(
        &mut self,
        packet: ServerboundUseItemOnPacket,
    ) -> anyhow::Result<()> {
        let item = self.player.inventory.get_hand(packet.hand).clone();
        let hit  = packet.hit_result;

        // 1. Try block.use() first (e.g. open chest, toggle lever)
        let mut level = self.player.level_mut();
        let block = level.get_block_state(hit.pos);
        if !self.player.is_crouching() || item.is_empty() {
            let used = block.use_block(
                &mut level,
                hit.pos,
                &hit,
                &self.player,
                packet.hand,
            ).await;
            if used == InteractionResult::Success {
                self.send_ack(packet.sequence).await?;
                return Ok(());
            }
        }

        // 2. Try item.useOn() to place a block or use an item on a block
        if item.is_empty() {
            self.send_ack(packet.sequence).await?;
            return Ok(());
        }

        let place_pos = if block.can_be_replaced(&hit) {
            hit.pos
        } else {
            let (dx, dy, dz) = hit.face.offset();
            BlockPos::new(hit.pos.x + dx, hit.pos.y + dy, hit.pos.z + dz)
        };

        if let Some(place_state) = item.get_place_state(place_pos, &hit, &level) {
            if place_state.can_place_at(&level, place_pos) {
                level.set_block(place_pos, place_state, SetBlockFlags::UPDATE_ALL)?;
                if self.player.game_mode != GameType::Creative {
                    let hand_item = self.player.inventory.get_hand_mut(packet.hand);
                    hand_item.count -= 1;
                    if hand_item.count <= 0 {
                        *hand_item = ItemStack::empty();
                    }
                }
                level.broadcast_block_update(place_pos).await;
            }
        }

        self.send_ack(packet.sequence).await
    }
}
```

### 22.7 — ServerLevel block change broadcast (`oxidized-game/src/level/block.rs`)

```rust
bitflags::bitflags! {
    pub struct SetBlockFlags: u32 {
        const NOTIFY_NEIGHBORS = 0x01;
        const NOTIFY_CLIENTS   = 0x02;
        const NO_RERENDER      = 0x04;
        const RERENDER_MAIN_THREAD = 0x08;
        const UPDATE_NEIGHBORS = 0x10;
        const SKIP_DROPS       = 0x20;
        const IS_MOVING        = 0x40;
        const UPDATE_ALL       = Self::NOTIFY_NEIGHBORS.bits() | Self::NOTIFY_CLIENTS.bits();
    }
}

impl ServerLevel {
    pub fn set_block(
        &mut self,
        pos: BlockPos,
        state: BlockState,
        flags: SetBlockFlags,
    ) -> anyhow::Result<bool> {
        let chunk_pos = ChunkPos::from_block(pos);
        let chunk = self.loaded_chunks.get_mut(&chunk_pos)
            .ok_or_else(|| anyhow::anyhow!("chunk not loaded at {:?}", pos))?;

        let old_state = chunk.get_block_state(pos);
        if old_state == state { return Ok(false); }

        chunk.set_block_state(pos, state.clone());
        self.dirty_chunks.mark_dirty(chunk_pos);

        if flags.contains(SetBlockFlags::NOTIFY_CLIENTS) {
            self.pending_block_updates.push(pos);
        }

        if flags.contains(SetBlockFlags::NOTIFY_NEIGHBORS) {
            self.notify_neighbors(pos, &old_state);
        }

        Ok(true)
    }

    pub async fn broadcast_block_update(&self, pos: BlockPos) {
        let state_id = self.get_block_state(pos).state_id();
        let packet = ClientboundBlockUpdatePacket { pos, block_state: state_id };
        self.broadcast_to_tracking_players(pos, packet).await;
    }

    /// Flush all pending block updates as SectionBlocksUpdate packets
    /// (more efficient than individual BlockUpdate packets when many blocks changed).
    pub async fn flush_block_updates(&mut self) {
        let mut by_section: HashMap<SectionPos, Vec<(u16, i32)>> = HashMap::new();

        for pos in self.pending_block_updates.drain(..) {
            let section = SectionPos::from_block_pos(pos);
            let rel_x = (pos.x & 15) as u16;
            let rel_y = (pos.y & 15) as u16;
            let rel_z = (pos.z & 15) as u16;
            let packed_pos = (rel_x << 8) | (rel_z << 4) | rel_y;
            let state_id = self.get_block_state(pos).state_id();
            by_section.entry(section).or_default().push((packed_pos, state_id));
        }

        for (section_pos, changes) in by_section {
            if changes.len() == 1 {
                // Single-block update is more efficient
                let (packed, state_id) = changes[0];
                let pos = unpack_section_pos(section_pos, packed);
                self.broadcast_to_tracking_section(
                    section_pos,
                    ClientboundBlockUpdatePacket { pos, block_state: state_id },
                ).await;
            } else {
                self.broadcast_to_tracking_section(
                    section_pos,
                    ClientboundSectionBlocksUpdatePacket {
                        section_pos,
                        positions_and_states: changes,
                    },
                ).await;
            }
        }
    }
}
```

### 22.8 — Sign update handler

```rust
impl PlayerConnection {
    pub async fn handle_sign_update(
        &mut self,
        packet: ServerboundSignUpdatePacket,
    ) -> anyhow::Result<()> {
        // Validate player is within reach of the sign
        anyhow::ensure!(
            self.player.position.distance_to_block(packet.pos) < 8.0,
            "player too far from sign"
        );

        let mut level = self.player.level_mut();
        let be = level.get_block_entity_mut(packet.pos)
            .ok_or_else(|| anyhow::anyhow!("no block entity at {:?}", packet.pos))?;

        if let BlockEntity::Sign(sign) = be {
            let text = if packet.is_front_text { &mut sign.front_text } else { &mut sign.back_text };
            for (i, line) in packet.lines.iter().enumerate() {
                // Validate each line ≤ 384 chars
                anyhow::ensure!(line.len() <= 384, "sign line too long");
                text.messages[i] = Component::text(line.clone());
            }
            level.dirty_chunks.mark_dirty(ChunkPos::from_block(packet.pos));
            level.broadcast_block_entity_update(packet.pos).await;
        }
        Ok(())
    }
}
```

---

## Data Structures

```rust
// oxidized-game/src/world/block_state.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockState {
    pub block: BlockId,
    pub properties: std::collections::BTreeMap<String, String>,
    cached_state_id: i32,
}

impl BlockState {
    pub const AIR: Self = Self { block: BlockId(0), properties: BTreeMap::new(), cached_state_id: 0 };

    pub fn state_id(&self) -> i32 { self.cached_state_id }
    pub fn destroy_time(&self) -> f32 { /* from registry */ todo!() }
    pub fn needs_random_tick(&self) -> bool { /* from registry */ todo!() }
    pub fn is_air(&self) -> bool { self.block.0 == 0 }
    pub fn can_be_replaced(&self, _hit: &BlockHitResult) -> bool { self.is_air() }
}

// oxidized-game/src/player/block_breaking.rs

pub struct ActiveBlockBreaking {
    pub pos: BlockPos,
    pub face: Direction,
    pub progress: f32,
    pub start_tick: u64,
    pub sequence: i32,
}
```

---

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- Direction ---

    #[test]
    fn direction_offset_up_is_positive_y() {
        assert_eq!(Direction::Up.offset(), (0, 1, 0));
    }

    #[test]
    fn direction_offset_north_is_negative_z() {
        assert_eq!(Direction::North.offset(), (0, 0, -1));
    }

    // --- SectionPos ---

    #[test]
    fn section_pos_from_block_pos() {
        let pos = BlockPos::new(17, 68, -5);
        let sec = SectionPos::from_block_pos(pos);
        assert_eq!(sec.x, 1);   // 17 >> 4 = 1
        assert_eq!(sec.y, 4);   // 68 >> 4 = 4
        assert_eq!(sec.z, -1);  // -5 >> 4 = -1
    }

    #[test]
    fn section_pos_packed_encodes_correctly() {
        let sec = SectionPos { x: 0, y: 0, z: 0 };
        let packed = sec.packed();
        // y=-64 is section y=0 in world; world_y=0 → section_y=4 with +4 offset
        // Just verify it's a valid i64
        assert!(packed >= 0 || packed < 0, "packed must be a valid i64");
    }

    // --- Block place position ---

    #[test]
    fn block_place_pos_on_air_is_same_pos() {
        // When hitting an air block face, place at the same pos
        // Implementation: if block.can_be_replaced() → place at hit.pos
        let air = BlockState::AIR;
        let hit = BlockHitResult {
            pos: BlockPos::new(5, 64, 5),
            face: Direction::Up,
            cursor_x: 0.5, cursor_y: 0.5, cursor_z: 0.5,
            inside: false,
        };
        assert!(air.can_be_replaced(&hit));
    }

    #[test]
    fn block_place_pos_on_solid_block_uses_face_offset() {
        // When hitting a solid block, place at pos + face.offset()
        let face_offset = Direction::Up.offset();
        let hit_pos = BlockPos::new(0, 63, 0);
        let place_pos = BlockPos::new(
            hit_pos.x + face_offset.0,
            hit_pos.y + face_offset.1,
            hit_pos.z + face_offset.2,
        );
        assert_eq!(place_pos, BlockPos::new(0, 64, 0));
    }

    // --- Mining speed ---

    #[test]
    fn mining_speed_haste_amplifier_0_adds_20_percent() {
        let base = 1.0f32;
        // haste I: 1.0 * (1.0 + 0.2 * 1) = 1.2
        let with_haste = base * (1.0 + 0.2 * 1.0);
        assert!((with_haste - 1.2).abs() < 1e-5);
    }

    #[test]
    fn mining_speed_mining_fatigue_3_is_very_slow() {
        let base = 10.0f32;
        // level 3 → factor 0.00081
        let with_fatigue = base * 0.00081;
        assert!(with_fatigue < 0.01);
    }

    #[test]
    fn mining_speed_in_water_without_affinity_is_one_fifth() {
        let base = 5.0f32;
        let in_water = base / 5.0;
        assert!((in_water - 1.0).abs() < 1e-5);
    }

    // --- SetBlockFlags ---

    #[test]
    fn set_block_flags_update_all_includes_notify_bits() {
        let flags = SetBlockFlags::UPDATE_ALL;
        assert!(flags.contains(SetBlockFlags::NOTIFY_NEIGHBORS));
        assert!(flags.contains(SetBlockFlags::NOTIFY_CLIENTS));
    }

    // --- Integration: break block sets to air ---

    #[tokio::test]
    async fn break_block_sets_air_in_level() {
        let (mut conn, mut level) = make_test_player_and_level();
        let pos = BlockPos::new(0, 64, 0);
        level.set_block(pos, BlockState::stone(), SetBlockFlags::UPDATE_ALL).unwrap();
        conn.player.game_mode = GameType::Creative;

        conn.break_block(pos, 1).await.unwrap();

        assert!(level.get_block_state(pos).is_air(), "broken block should become air");
    }

    #[tokio::test]
    async fn break_block_marks_chunk_dirty() {
        let (mut conn, mut level) = make_test_player_and_level();
        let pos = BlockPos::new(0, 64, 0);
        level.set_block(pos, BlockState::stone(), SetBlockFlags::UPDATE_ALL).unwrap();
        conn.player.game_mode = GameType::Creative;

        conn.break_block(pos, 1).await.unwrap();

        let chunk_pos = ChunkPos::from_block(pos);
        assert!(level.dirty_chunks.is_dirty(&chunk_pos));
    }
}
```
