//! How values are laid out in 64-bit slots, for the register VM and native
//! backends. The reference interpreter does not use it; it fixes the contract.
//!
//! - unit, bool, every number: one slot holding the value (floats as bits,
//!   `f32` widened to `f64`, `u64` as its bit pattern).
//! - strings, lists, maps, tuples, structs, enums, closures: one slot holding
//!   a pointer to a heap object. Objects are reference counted and copied
//!   before a write when shared.
//! - a heap object for a struct, tuple or closure is a header plus one slot per
//!   field; an enum is a header, a tag slot and as many payload slots as its
//!   largest variant needs.
//!
//! A `Take` leaves an empty marker in the slot it took from.

use super::ty::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repr {
    /// Lives in the register slot itself.
    Scalar,
    /// The register slot points at a heap object.
    Heap,
}

pub fn repr(types: &TypeTable, ty: TyId) -> Repr {
    match types.kind(ty) {
        TyKind::Unit | TyKind::Bool | TyKind::Num(_) => Repr::Scalar,
        _ => Repr::Heap,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdtLayout {
    /// Slot of the variant tag inside the object; enums only.
    pub tag_slot: Option<u32>,
    /// Payload slots (all variants share the same area); for a struct, its fields.
    pub payload_slots: u32,
    /// For enums, the number of payload slots each variant uses.
    pub variant_slots: Vec<u32>,
}

/// Slots of a struct or enum object, excluding its header.
pub fn adt_layout(types: &TypeTable, adt: AdtId) -> AdtLayout {
    match &types.adt(adt).body {
        AdtBody::Struct(fields) => AdtLayout {
            tag_slot: None,
            payload_slots: fields.len() as u32,
            variant_slots: Vec::new(),
        },
        AdtBody::Enum(variants) => {
            let variant_slots: Vec<u32> = variants.iter().map(|v| v.fields.len() as u32).collect();
            AdtLayout {
                tag_slot: Some(0),
                payload_slots: variant_slots.iter().copied().max().unwrap_or(0) + 1,
                variant_slots,
            }
        }
    }
}

/// Slot index of `field` of `variant` inside an enum object (after the tag).
pub fn variant_field_slot(field: u32) -> u32 {
    field + 1
}
