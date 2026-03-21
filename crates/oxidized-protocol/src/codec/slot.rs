//! Slot (item stack) wire encoding/decoding for the Minecraft protocol.
//!
//! In protocol version 26.1 (1.20.5+), item slots use the "optional item" format:
//! - `VarInt count` — 0 means empty slot, >0 means item present
//! - `VarInt item_id` — registry ID of the item (only if count > 0)
//! - `DataComponentPatch` — component modifications (only if count > 0)
//!
//! The `DataComponentPatch` wire format is:
//! - `VarInt added_count` — number of components added/modified
//! - `VarInt removed_count` — number of components removed
//! - For each added: `VarInt type_id` + type-specific encoded value
//! - For each removed: `VarInt type_id`

use bytes::{Bytes, BytesMut};

use super::packet::PacketDecodeError;
use super::varint;

/// A decoded item slot from the wire.
///
/// Contains the raw data needed to reconstruct an `ItemStack` at the game layer.
/// The item is identified by its numeric registry ID on the wire; the game layer
/// maps this to a resource location string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotData {
    /// Number of items in the stack (1–99).
    pub count: i32,
    /// Item registry numeric ID.
    pub item_id: i32,
    /// Raw component patch bytes. For now, always empty (0 added, 0 removed).
    /// Future phases will parse these into typed component values.
    pub component_data: ComponentPatchData,
}

/// Raw data component patch — counts only for now.
///
/// In the future this will hold parsed component entries. For Phase 21,
/// we only support the "no modifications" case (0 added, 0 removed).
/// Byte storage fields will be added when Phase 29 implements component parsing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentPatchData {
    /// Number of added/modified components.
    pub added_count: i32,
    /// Number of removed components.
    pub removed_count: i32,
}

/// Writes an optional item slot to the buffer.
///
/// `None` or a slot with count ≤ 0 writes a single `VarInt(0)` (empty slot).
///
/// # Examples
///
/// ```
/// use bytes::BytesMut;
/// use oxidized_protocol::codec::slot::{write_slot, SlotData, ComponentPatchData};
///
/// let mut buf = BytesMut::new();
/// write_slot(&mut buf, None); // empty slot
/// assert_eq!(buf.len(), 1); // VarInt(0) = 1 byte
/// ```
pub fn write_slot(buf: &mut BytesMut, slot: Option<&SlotData>) {
    match slot {
        None => {
            varint::write_varint_buf(0, buf);
        },
        Some(data) if data.count <= 0 => {
            varint::write_varint_buf(0, buf);
        },
        Some(data) => {
            varint::write_varint_buf(data.count, buf);
            varint::write_varint_buf(data.item_id, buf);
            write_component_patch(buf, &data.component_data);
        },
    }
}

/// Reads an optional item slot from the buffer.
///
/// Returns `None` for an empty slot (count = 0).
///
/// # Errors
///
/// Returns [`PacketDecodeError`] if the buffer is truncated.
///
/// # Examples
///
/// ```
/// use bytes::{Bytes, BytesMut};
/// use oxidized_protocol::codec::slot::{write_slot, read_slot};
///
/// let mut buf = BytesMut::new();
/// write_slot(&mut buf, None);
/// let mut data = buf.freeze();
/// let result = read_slot(&mut data).unwrap();
/// assert!(result.is_none());
/// ```
pub fn read_slot(buf: &mut Bytes) -> Result<Option<SlotData>, PacketDecodeError> {
    let count = varint::read_varint_buf(buf)?;
    if count <= 0 {
        return Ok(None);
    }

    let item_id = varint::read_varint_buf(buf)?;
    let component_data = read_component_patch(buf)?;

    Ok(Some(SlotData {
        count,
        item_id,
        component_data,
    }))
}

/// Writes a `DataComponentPatch` to the buffer.
fn write_component_patch(buf: &mut BytesMut, patch: &ComponentPatchData) {
    varint::write_varint_buf(patch.added_count, buf);
    varint::write_varint_buf(patch.removed_count, buf);
    // TODO(Phase 29+): Write actual component entries when component registry exists.
}

/// Reads a `DataComponentPatch` from the buffer.
///
/// For now, only supports the empty patch case (0 added, 0 removed).
/// Returns an error if the client sends non-zero component counts, since
/// we cannot safely skip variable-length component entries without a
/// component type registry.
fn read_component_patch(buf: &mut Bytes) -> Result<ComponentPatchData, PacketDecodeError> {
    let added_count = varint::read_varint_buf(buf)?;
    let removed_count = varint::read_varint_buf(buf)?;

    // TODO(Phase 29+): Parse actual component entries when component registry exists.
    // Without a registry we cannot determine the byte length of each entry,
    // so reject packets with components rather than desync the stream.
    if added_count != 0 || removed_count != 0 {
        return Err(PacketDecodeError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!(
                "data component patch with {added_count} added / {removed_count} removed \
                 not yet supported (Phase 21)"
            ),
        )));
    }

    Ok(ComponentPatchData {
        added_count: 0,
        removed_count: 0,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_write_read_empty_slot() {
        let mut buf = BytesMut::new();
        write_slot(&mut buf, None);
        let mut data = buf.freeze();
        let result = read_slot(&mut data).unwrap();
        assert!(result.is_none());
        assert_eq!(data.len(), 0, "all bytes consumed");
    }

    #[test]
    fn test_write_read_slot_roundtrip() {
        let slot = SlotData {
            count: 64,
            item_id: 1,
            component_data: ComponentPatchData::default(),
        };

        let mut buf = BytesMut::new();
        write_slot(&mut buf, Some(&slot));
        let mut data = buf.freeze();
        let result = read_slot(&mut data).unwrap().unwrap();

        assert_eq!(result.count, 64);
        assert_eq!(result.item_id, 1);
        assert_eq!(result.component_data.added_count, 0);
        assert_eq!(result.component_data.removed_count, 0);
        assert_eq!(data.len(), 0, "all bytes consumed");
    }

    #[test]
    fn test_write_read_single_item() {
        let slot = SlotData {
            count: 1,
            item_id: 42,
            component_data: ComponentPatchData::default(),
        };

        let mut buf = BytesMut::new();
        write_slot(&mut buf, Some(&slot));
        let mut data = buf.freeze();
        let result = read_slot(&mut data).unwrap().unwrap();

        assert_eq!(result.count, 1);
        assert_eq!(result.item_id, 42);
    }

    #[test]
    fn test_zero_count_slot_is_empty() {
        let slot = SlotData {
            count: 0,
            item_id: 1,
            component_data: ComponentPatchData::default(),
        };

        let mut buf = BytesMut::new();
        write_slot(&mut buf, Some(&slot));
        let mut data = buf.freeze();
        let result = read_slot(&mut data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_negative_count_slot_is_empty() {
        let slot = SlotData {
            count: -1,
            item_id: 1,
            component_data: ComponentPatchData::default(),
        };

        let mut buf = BytesMut::new();
        write_slot(&mut buf, Some(&slot));
        let mut data = buf.freeze();
        let result = read_slot(&mut data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_multiple_slots_sequential() {
        let mut buf = BytesMut::new();

        write_slot(&mut buf, None);
        write_slot(
            &mut buf,
            Some(&SlotData {
                count: 32,
                item_id: 10,
                component_data: ComponentPatchData::default(),
            }),
        );
        write_slot(&mut buf, None);

        let mut data = buf.freeze();
        assert!(read_slot(&mut data).unwrap().is_none());
        let s = read_slot(&mut data).unwrap().unwrap();
        assert_eq!(s.count, 32);
        assert_eq!(s.item_id, 10);
        assert!(read_slot(&mut data).unwrap().is_none());
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_nonzero_component_patch_rejected() {
        // Craft a raw slot with count=1, item_id=1, added_count=1 (unsupported).
        let mut buf = BytesMut::new();
        varint::write_varint_buf(1, &mut buf); // count
        varint::write_varint_buf(1, &mut buf); // item_id
        varint::write_varint_buf(1, &mut buf); // added_count = 1 (unsupported)
        varint::write_varint_buf(0, &mut buf); // removed_count

        let mut data = buf.freeze();
        let result = read_slot(&mut data);
        assert!(
            result.is_err(),
            "non-zero component counts should be rejected"
        );
    }
}
