//! What the generated code and the runtime (`tint-wasmrt`) agree on.
//!
//! Heap values are pointers (`u32`, addresses in the runtime's memory) to
//! objects that begin with a header `{ rc: u32, kind: u32 }`. The generated
//! code reads and writes the fields below directly on its fast paths and calls
//! the runtime for everything else.
//!
//! - `Obj` (struct, tuple, enum, closure): `nslots: u32` at 8, pointer mask
//!   `u64` at 16 (bit `i` set: slot `i` owns a pointer), slots from 24, eight
//!   bytes each. A scalar slot holds the value's 64-bit bits, a pointer slot
//!   the address zero-extended. An enum keeps its variant tag in slot 0, a
//!   closure its function id.
//! - `List`: `len: u32` at 8, `cap: u32` at 12, `ptr: u32` (elements) at 16,
//!   `esize: u32` at 20, `heap: u32` at 24. Scalar elements are stored raw in
//!   `esize` bytes, heap elements are 8-byte slots holding owned pointers.
//! - `Str`, `Map`: private to the runtime.

use tint_ir::typed::*;

mod descriptor;
mod layout;

pub const KIND_LIST: u32 = 1;
pub const KIND_OBJ: u32 = 2;
pub const KIND_STR: u32 = 3;
pub const KIND_MAP: u32 = 4;

/// Reference count of objects that are never freed (literals, fieldless variants).
pub const IMMORTAL: u32 = 1 << 31;

pub const OFF_RC: u64 = 0;
pub const OFF_NSLOTS: u64 = 8;
pub const OFF_MASK: u64 = 16;
pub const OFF_SLOTS: u64 = 24;
pub const OFF_LEN: u64 = 8;
pub const OFF_CAP: u64 = 12;
pub const OFF_PTR: u64 = 16;
pub const OFF_ESIZE: u64 = 20;
pub const OFF_HEAP: u64 = 24;

pub const KINDS: [NumKind; 8] = [
    NumKind::Num,
    NumKind::I32,
    NumKind::I64,
    NumKind::U8,
    NumKind::U32,
    NumKind::U64,
    NumKind::F32,
    NumKind::F64,
];

pub fn kind_index(k: NumKind) -> u32 {
    KINDS.iter().position(|x| *x == k).unwrap() as u32
}

pub const CMP_OPS: [CmpOp; 6] = [
    CmpOp::Eq,
    CmpOp::Ne,
    CmpOp::Lt,
    CmpOp::Le,
    CmpOp::Gt,
    CmpOp::Ge,
];

/// The runtime calls that are executed by the reference implementation; the
/// index is what `call` receives.
pub const RT_FNS: &[RtFn] = &[
    RtFn::StrConcat,
    RtFn::StrLen,
    RtFn::StrIsEmpty,
    RtFn::StrTrim,
    RtFn::StrUpper,
    RtFn::StrLower,
    RtFn::StrContains,
    RtFn::StrStartsWith,
    RtFn::StrEndsWith,
    RtFn::StrSlice,
    RtFn::StrSplit,
    RtFn::StrReplace,
    RtFn::ListLen,
    RtFn::ListIsEmpty,
    RtFn::ListPush,
    RtFn::ListPop,
    RtFn::ListRemove,
    RtFn::ListReverse,
    RtFn::ListSort,
    RtFn::ListSlice,
    RtFn::ListContains,
    RtFn::ListJoin,
    RtFn::MapLen,
    RtFn::MapIsEmpty,
    RtFn::MapHas,
    RtFn::MapGet,
    RtFn::MapSet,
    RtFn::MapRemove,
    RtFn::MapKeys,
    RtFn::MapValues,
    RtFn::Sqrt,
    RtFn::Min,
    RtFn::Max,
    RtFn::Abs,
    RtFn::Sign,
    RtFn::Clamp,
    RtFn::ParseNumber,
    RtFn::Vec2Length,
    RtFn::Vec2Normalized,
    RtFn::LineCount,
    RtFn::MaxLineLen,
    RtFn::LineNumbers,
    RtFn::Highlight,
    RtFn::PropsJson,
];

pub fn rt_fn_index(f: RtFn) -> u32 {
    RT_FNS.iter().position(|x| *x == f).expect("listed") as u32
}

pub use descriptor::{decode, encode};
pub use layout::{adt_slots, elem_layout, is_heap, ptr_mask, ElemLayout};
