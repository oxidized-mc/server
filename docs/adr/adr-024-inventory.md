# ADR-024: Inventory & Container Transactions

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-17 |
| Phases | P21, P29, P30 |
| Deciders | Oxidized Core Team |

## Context

Minecraft's inventory system is one of the most interaction-heavy subsystems in the game.
Every time a player opens a chest, crafting table, furnace, anvil, enchanting table,
brewing stand, villager trade menu, or their own inventory, a **container menu** (called
`AbstractContainerMenu` in vanilla) is created. This menu maps protocol-level slot IDs to
internal storage slots, handles all click operations (left click, right click, shift-click,
number key swap, drop, drag-select, double-click-collect), validates operations, and
synchronizes state between server and client. The protocol uses a `state_id` counter for
optimistic synchronization: the client sends its known state_id with each click, and if it
doesn't match the server's, the server resynchronizes the entire container.

The click operation semantics are surprisingly complex. A left-click on a non-empty slot
with an empty cursor picks up the stack. A left-click on a non-empty slot with a
non-empty cursor swaps if incompatible or merges if compatible (up to max stack size). A
right-click picks up half. A shift-click moves the stack to the "other" section of the
container (player inventory → container slots, or vice versa), following section-specific
routing rules (shift-click in a furnace result slot goes to player inventory, not the fuel
slot). Drag mode (click-and-drag across multiple slots) distributes items evenly (left drag)
or one-per-slot (right drag). Double-click collects all matching items into the cursor
stack. Each of these operations must leave the container in a valid state: no negative
counts, no stacks exceeding max size, no items appearing from nowhere (duplication), no
items disappearing (loss).

Duplication glitches are among the most exploited bugs in Minecraft's history. They
typically arise from race conditions: a click is processed between state changes, or the
client's state_id diverges from the server's in a way that creates items. Vanilla has
patched dozens of these over the years, but the architecture (mutable slot arrays with
per-operation validation) makes them inherently fragile. Oxidized can do better by using a
transactional model where operations are validated atomically.

## Decision Drivers

- **Duplication prevention**: The inventory system must make item duplication impossible by
  construction, not by per-exploit patching.
- **Protocol compatibility**: Slot ID mapping, state_id synchronization, and all click
  operation semantics must match vanilla's protocol behavior exactly.
- **Operation correctness**: Every click type (left, right, shift, swap, drop, drag,
  double-click) must produce the same result as vanilla for any container state.
- **Performance**: Inventory operations happen in response to player clicks (human speed,
  ~10 per second max). Performance is not a primary concern, but the system must not block
  the tick loop.
- **Container variety**: The system must support all container types: player inventory,
  chests (single/double), shulker boxes, crafting tables (2x2 and 3x3), furnaces (regular,
  blast, smoker), brewing stands, anvils, enchanting tables, grindstones, stonecutters,
  looms, cartography tables, smithing tables, villager trades, horse/llama inventories,
  hoppers, dispensers/droppers, beacon, creative inventory.
- **Recipe integration**: Crafting, smelting, and smithing operations must integrate with
  the recipe system to validate outputs and consume inputs.

## Considered Options

### Option 1: Mutable Slot Arrays Like Vanilla

Directly replicate vanilla's model: containers are arrays of `ItemStack` slots, and each
click operation directly mutates the slots.

**Pros:**
- Direct mapping to vanilla source.
- Simple mental model — slots are just an array.

**Cons:**
- Vulnerable to the same class of duplication bugs that plague vanilla.
- Operations are not atomic — a crash or error mid-operation can leave the container in
  an inconsistent state (items in cursor + items in slot = more items than before).
- Validation is after-the-fact: mutate first, then check if the result is valid.

**Verdict: Rejected.** We can preserve vanilla's behavior while improving the implementation.

### Option 2: Transactional Model (Begin → Modify → Commit/Rollback)

Each container operation begins a transaction, makes modifications to a working copy of
the slot data, validates the entire result, and commits atomically. If validation fails,
the transaction is rolled back and the client receives a full resynchronization.

**Pros:**
- Atomic operations — no partial state changes visible to other systems.
- Validation against the complete final state, not intermediate states.
- Rollback is trivial — discard the working copy.
- Duplication is prevented structurally — the transaction validator checks that total
  item counts are conserved.

**Cons:**
- Slight overhead: copying slot data into a working buffer for each operation. For a
  typical container (27 slots), this is a memcpy of ~27 × sizeof(ItemStack) ≈ 2-4 KB.
  Negligible.
- Must ensure the transaction model produces identical observable results to vanilla's
  direct mutation model (same final slot states, same state_id behavior).

**Verdict: Selected.** Principled duplication prevention with negligible cost.

### Option 3: Event-Sourced (Log of Operations → Derive State)

Store inventory as a log of operations (insert, remove, swap, merge). Current state is
derived by replaying the log. New operations append to the log.

**Pros:**
- Full audit trail — can replay any historical state.
- Naturally supports undo/redo.

**Cons:**
- Deriving current state requires replaying the full log (or maintaining snapshots).
- Log grows unboundedly without compaction.
- Overkill for an inventory system — we don't need audit trails.
- Does not naturally prevent duplication (a malicious log entry can create items).

**Verdict: Rejected.** Wrong abstraction for real-time inventory management.

### Option 4: CRDT-Based Convergent State

Use Conflict-free Replicated Data Types to handle client-server state divergence. Both
client and server maintain replicas that converge without coordination.

**Pros:**
- Elegant handling of network latency and out-of-order operations.

**Cons:**
- CRDTs for ordered slot-based inventories are complex and not well-studied.
- Minecraft's protocol already handles divergence via state_id resync — CRDTs are
  solving a problem that doesn't exist.
- Significant implementation complexity for no practical benefit.

**Verdict: Rejected.** Overengineered solution with no practical benefit for Minecraft's
synchronization model.

## Decision

**We use a transactional slot modification model with optimistic locking via state_id.**
Each container operation creates a `ContainerTransaction` that snapshots the relevant
slots, applies modifications, validates the result (conservation of items, valid stack
sizes, recipe validity), and commits atomically. If validation fails, the transaction is
rolled back and the client receives a full `ClientboundContainerSetContentPacket`.

### ItemStack Representation

```rust
#[derive(Clone, Debug, PartialEq)]
struct ItemStack {
    /// The item type (e.g., minecraft:diamond_sword).
    item: ResourceLocation,
    /// Stack count (1-99 for most items, 1 for tools/armor).
    count: u8,
    /// Data components (enchantments, damage, custom name, lore, etc.).
    /// Replaces the old NBT tag system as of 1.20.5+.
    components: DataComponentPatch,
}

impl ItemStack {
    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn max_stack_size(&self) -> u8 {
        // Look up from item registry; most items are 64, tools/armor are 1, etc.
        item_registry::max_stack_size(&self.item)
    }

    fn is_stackable_with(&self, other: &ItemStack) -> bool {
        self.item == other.item && self.components == other.components
    }
}

/// Empty slot constant.
const EMPTY: ItemStack = ItemStack {
    item: ResourceLocation::EMPTY,
    count: 0,
    components: DataComponentPatch::EMPTY,
};
```

### Container Menu Structure

```rust
struct ContainerMenu {
    /// Unique container ID sent to the client (1-255, cycling).
    container_id: u8,
    /// All slots in protocol order.
    slots: Vec<Slot>,
    /// The item currently held by the cursor (carried item).
    carried: ItemStack,
    /// Optimistic lock counter — incremented on every server-side change.
    state_id: i32,
    /// Menu type (generic 9x3, furnace, crafting, etc.).
    menu_type: MenuType,
    /// Quick-craft (drag) state machine.
    quick_craft: QuickCraftState,
}

struct Slot {
    /// The stored item stack.
    item: ItemStack,
    /// Which container this slot belongs to (player inventory, chest, etc.).
    container: ContainerRef,
    /// Index within the container.
    container_index: usize,
    /// Whether this slot accepts the given item (e.g., fuel slot only accepts fuel).
    predicate: Option<Box<dyn Fn(&ItemStack) -> bool + Send + Sync>>,
    /// Maximum stack size for this slot (may differ from item's max stack size).
    max_stack_size: u8,
}
```

### Slot ID Mapping

Protocol slot IDs are globally sequential across all containers in the menu. For a
standard chest (27 slots) opened by a player:

| Protocol Slot IDs | Container | Purpose |
|-------------------|-----------|---------|
| 0-26 | Chest | 27 chest slots (top-left to bottom-right) |
| 27-53 | Player Main Inventory | 27 main inventory slots |
| 54-62 | Player Hotbar | 9 hotbar slots |

For a player's own inventory screen:

| Protocol Slot IDs | Container | Purpose |
|-------------------|-----------|---------|
| 0 | Crafting Output | Result of 2x2 crafting |
| 1-4 | Crafting Grid | 2x2 crafting input |
| 5-8 | Armor Slots | Head, Chest, Legs, Feet |
| 9-35 | Player Main Inventory | 27 main inventory slots |
| 36-44 | Player Hotbar | 9 hotbar slots |
| 45 | Offhand | Shield/offhand slot |

Each `MenuType` defines its slot mapping. The mapping is hardcoded per type to match
vanilla's exact layout.

### Transaction Model

```rust
struct ContainerTransaction {
    /// Snapshot of slot states before the operation.
    slot_snapshots: SmallVec<[(usize, ItemStack); 8]>,
    /// Snapshot of carried item before the operation.
    carried_snapshot: ItemStack,
    /// Modifications to apply.
    modifications: Vec<SlotModification>,
}

enum SlotModification {
    SetSlot { index: usize, item: ItemStack },
    SetCarried { item: ItemStack },
}

impl ContainerMenu {
    fn begin_transaction(&self) -> ContainerTransaction {
        ContainerTransaction {
            slot_snapshots: SmallVec::new(),
            carried_snapshot: self.carried.clone(),
            modifications: Vec::new(),
        }
    }

    fn commit_transaction(&mut self, tx: ContainerTransaction) -> Result<(), ContainerError> {
        // 1. Validate conservation: total item count before == total item count after
        // 2. Validate stack sizes: no slot exceeds its max
        // 3. Validate slot predicates: each slot accepts its new item
        // 4. Apply all modifications atomically
        // 5. Increment state_id

        self.validate_conservation(&tx)?;
        self.validate_constraints(&tx)?;

        for modification in &tx.modifications {
            match modification {
                SlotModification::SetSlot { index, item } => {
                    self.slots[*index].item = item.clone();
                }
                SlotModification::SetCarried { item } => {
                    self.carried = item.clone();
                }
            }
        }

        self.state_id = self.state_id.wrapping_add(1);
        Ok(())
    }

    fn rollback_and_resync(&mut self, player_bridge: &NetworkBridge) {
        // Send ClientboundContainerSetContentPacket with full current state
        player_bridge.send(OutboundPacket::ContainerSetContent {
            container_id: self.container_id,
            state_id: self.state_id,
            items: self.slots.iter().map(|s| s.item.clone()).collect(),
            carried: self.carried.clone(),
        });
    }
}
```

### Click Operation Handlers

Each click type is implemented as a function that creates and populates a transaction:

```rust
fn handle_click(
    menu: &mut ContainerMenu,
    slot_id: i32,      // -999 for outside window, 0+ for slot
    button: i32,       // 0=left, 1=right, 2+=other
    click_type: ClickType,
    client_state_id: i32,
) -> Result<(), ContainerError> {
    // Optimistic lock check
    if client_state_id != menu.state_id {
        // Client is out of sync — resync immediately
        menu.rollback_and_resync(bridge);
        return Ok(());
    }

    let mut tx = menu.begin_transaction();

    match click_type {
        ClickType::Pickup => handle_pickup(&mut tx, menu, slot_id, button),
        ClickType::QuickMove => handle_quick_move(&mut tx, menu, slot_id, button),
        ClickType::Swap => handle_swap(&mut tx, menu, slot_id, button),
        ClickType::Clone => handle_clone(&mut tx, menu, slot_id), // creative only
        ClickType::Throw => handle_throw(&mut tx, menu, slot_id, button),
        ClickType::QuickCraft => handle_quick_craft(&mut tx, menu, slot_id, button),
        ClickType::PickupAll => handle_pickup_all(&mut tx, menu, slot_id, button),
    }

    menu.commit_transaction(tx)?;
    Ok(())
}
```

### Quick-Move (Shift-Click) Routing

Shift-click moves items between container sections. The routing logic is container-specific:

```rust
fn handle_quick_move(
    tx: &mut ContainerTransaction,
    menu: &ContainerMenu,
    slot_id: i32,
    _button: i32,
) {
    let item = menu.slots[slot_id as usize].item.clone();
    if item.is_empty() { return; }

    let target_range = match menu.menu_type {
        MenuType::Generic9x3 => {
            if slot_id < 27 {
                // Chest slot → player inventory (27-62)
                27..63
            } else {
                // Player inventory → chest slots (0-26)
                0..27
            }
        }
        MenuType::Furnace => {
            match slot_id {
                0 => 3..39,    // Ingredient → player inventory
                1 => 3..39,    // Fuel → player inventory
                2 => 3..39,    // Result → player inventory
                3..=38 => {     // Player inventory → furnace
                    if is_fuel(&item) { 1..2 }         // Fuel slot
                    else if is_smeltable(&item) { 0..1 } // Ingredient slot
                    else { return; }                      // Neither — no shift-click target
                }
                _ => return,
            }
        }
        // ... other menu types
    };

    // Try to merge into existing stacks first, then empty slots
    merge_into_range(tx, menu, slot_id as usize, &item, target_range);
}
```

### Drag Mode State Machine

Drag (click-and-drag) distributes items across multiple slots:

```rust
enum QuickCraftState {
    Idle,
    /// Drag started. Button determines mode: 0=even split, 1=one each, 2=creative clone.
    Started { mode: DragMode, slots: Vec<usize> },
}

enum DragMode {
    EvenSplit,  // Left drag: distribute cursor evenly
    OneEach,    // Right drag: place one per slot
    Clone,      // Creative middle drag: clone to each slot
}
```

Drag works in three protocol messages:
1. **Start**: `button=0` (left start), `button=4` (right start), `button=8` (middle start)
2. **Add slot**: `button=1,5,9` with slot_id — adds slot to drag set
3. **End**: `button=2,6,10` — compute distribution and apply

### Creative Mode

Creative mode has special handling:
- Clicking in the creative inventory tab creates items from thin air (no conservation
  check for the creative palette).
- Middle-click on existing items clones the stack.
- The creative inventory palette is client-side only — the server receives
  `ServerboundSetCreativeModeSlotPacket` for explicit slot modifications.

### Network Serialization

ItemStack serialization for network protocol:

```rust
fn write_item_stack(buf: &mut BytesMut, stack: &ItemStack) {
    if stack.is_empty() {
        buf.write_var_int(0); // count = 0 means empty
        return;
    }

    buf.write_var_int(stack.count as i32);
    buf.write_var_int(item_registry::id_for(&stack.item));
    // Data components use ADD/REMOVE patch format
    stack.components.write_to(buf);
}
```

ItemStack serialization for NBT (persistence/player data):

```rust
fn write_item_stack_nbt(stack: &ItemStack) -> NbtCompound {
    let mut tag = NbtCompound::new();
    tag.put_string("id", stack.item.to_string());
    tag.put_byte("count", stack.count as i8);
    if !stack.components.is_empty() {
        tag.put("components", stack.components.to_nbt());
    }
    tag
}
```

## Consequences

### Positive

- **Duplication prevention by construction**: The transaction validator checks item count
  conservation before committing any change. Creating items from nothing is a validation
  error, not a bug to be patched per-exploit.
- **Atomic operations**: No intermediate states are visible. A crash during transaction
  processing simply discards the uncommitted transaction; the container state is the
  pre-transaction state.
- **Clear debugging**: If a click produces unexpected results, the transaction log shows
  exactly what modifications were attempted and whether they passed validation.
- **state_id resync as a safety net**: Even if the transaction model has a bug, the
  state_id mismatch detection triggers a full resync, preventing persistent desync.

### Negative

- **Transaction overhead**: Each operation copies affected slot data into a snapshot. For
  typical operations (1-3 slots affected), this is ~100 bytes of copying — negligible but
  measurable in microbenchmarks.
- **Quick-move routing complexity**: Shift-click routing is different for every container
  type. This is inherent complexity from vanilla that cannot be simplified.
- **Drag mode state machine**: The three-message drag protocol with six button values is
  complex and must be handled correctly for all edge cases (drag cancelled, drag with
  zero slots, etc.).

### Neutral

- **Recipe system integration**: Crafting output slots are computed by the recipe system
  whenever input slots change. This is a hook in the transaction commit — after modifying
  crafting input slots, recalculate the output. Not covered by this ADR.
- **Hopper interaction**: Hoppers moving items in/out of containers use the same slot
  modification API but bypass the click transaction system (hoppers don't "click" — they
  directly insert/extract). Hopper transfers still validate stack sizes and slot predicates.

## Compliance

- **Conservation test**: For every click type × container type combination, verify that
  total item count before the operation equals total item count after.
- **Vanilla parity test**: Replay recorded vanilla click sequences (captured from a vanilla
  client) and verify identical slot states after each operation.
- **Duplication fuzz test**: Send random sequences of click packets with random slot IDs,
  buttons, and click types. After each sequence, verify item count conservation.
- **state_id resync test**: Send a click with an incorrect state_id and verify the server
  responds with a full container resync.
- **Shift-click routing test**: For each container type, verify shift-click moves items to
  the correct target section (e.g., smeltable items go to furnace ingredient slot, not fuel).

## Related ADRs

- **ADR-018**: Entity System Architecture — player inventory is a set of ECS components
  on the player entity
- **ADR-020**: Player Session Lifecycle — container operations arrive as events through
  the network bridge

## References

- Vanilla source: `net.minecraft.world.inventory.AbstractContainerMenu`
- Vanilla source: `net.minecraft.world.inventory.AbstractContainerMenu.doClick()`
- Vanilla source: `net.minecraft.world.inventory.ChestMenu`
- Vanilla source: `net.minecraft.world.inventory.FurnaceMenu`
- Vanilla source: `net.minecraft.world.inventory.CraftingMenu`
- Vanilla source: `net.minecraft.world.item.ItemStack`
- Vanilla source: `net.minecraft.core.component.DataComponentPatch`
- [wiki.vg — Inventory](https://wiki.vg/Inventory) — protocol slot mapping
- [wiki.vg — Click Container](https://wiki.vg/Protocol#Click_Container) — click packet
- [Minecraft Wiki — Inventory](https://minecraft.wiki/w/Inventory)
