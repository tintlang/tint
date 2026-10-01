use tint_ir::typed::*;

/// How a list element is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElemLayout {
    /// Bytes per element.
    pub size: u32,
    /// Memory access: bytes loaded/stored.
    pub mem: u32,
    /// Sign-extend on load (only for narrower-than-register integers).
    pub signed: bool,
    /// The element is an owned pointer (stored in an 8-byte slot).
    pub heap: bool,
}

pub fn is_heap(m: &Module, ty: TyId) -> bool {
    !matches!(
        m.types.kind(ty),
        TyKind::Unit | TyKind::Bool | TyKind::Num(_)
    )
}

pub fn elem_layout(m: &Module, elem: TyId) -> ElemLayout {
    let s = |size, signed| ElemLayout {
        size,
        mem: size,
        signed,
        heap: false,
    };
    match m.types.kind(elem) {
        TyKind::Unit | TyKind::Bool => s(1, false),
        TyKind::Num(NumKind::U8) => s(1, false),
        TyKind::Num(NumKind::I32) => s(4, true),
        TyKind::Num(NumKind::U32) => s(4, false),
        TyKind::Num(_) => s(8, true),
        _ => ElemLayout {
            size: 8,
            mem: 8,
            signed: false,
            heap: true,
        },
    }
}

/// Bit `base + i` is set when `tys[i]` is a heap value.
pub fn ptr_mask(m: &Module, tys: &[TyId], base: usize) -> u64 {
    tys.iter()
        .enumerate()
        .filter(|(_, t)| is_heap(m, **t))
        .fold(0, |mask, (i, _)| mask | 1 << (base + i))
}

/// Slots of the object of `adt`, tag included for an enum.
pub fn adt_slots(m: &Module, adt: AdtId) -> u32 {
    tint_ir::typed::layout::adt_layout(&m.types, adt).payload_slots
}
