use cranelift_codegen::ir::{types, AbiParam, Type};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId as ClifFunc, Linkage, Module as _};
use std::collections::HashMap;
use tint_ir::typed::*;

#[derive(Debug, PartialEq)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported by the native backend: {}", self.0)
    }
}

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Result<T, Unsupported> {
    Err(Unsupported(what.into()))
}

/// A compiled module. Keeps the machine code alive.
pub struct Jit {
    jit: JITModule,
    ids: Vec<Option<ClifFunc>>,
    // Kept alive for the runtime callbacks.
    module: Box<Module>,
    msgs: Vec<String>,
    _fn_table: Box<[usize]>,
    _globals: Box<[u64]>,
    init_done: std::cell::Cell<bool>,
}

// ---- types -----------------------------------------------------------------

/// `Ok` when values of `ty` can live in native registers and objects.
fn check_ty(m: &Module, ty: TyId, seen: &mut Vec<TyId>) -> Result<(), Unsupported> {
    if seen.contains(&ty) {
        return Ok(());
    }
    match m.types.kind(ty) {
        TyKind::Unit | TyKind::Bool | TyKind::Num(_) | TyKind::Str => Ok(()),
        TyKind::List(e) => {
            seen.push(ty);
            check_ty(m, *e, seen)
        }
        TyKind::Map(v) => {
            seen.push(ty);
            check_ty(m, *v, seen)
        }
        TyKind::Fn(params, ret) => {
            seen.push(ty);
            params.iter().try_for_each(|t| check_ty(m, *t, seen))?;
            check_ty(m, *ret, seen)
        }
        TyKind::Tuple(items) => {
            seen.push(ty);
            items.iter().try_for_each(|t| check_ty(m, *t, seen))
        }
        TyKind::Adt(adt) => {
            seen.push(ty);
            match &m.types.adt(*adt).body {
                AdtBody::Struct(fields) => fields.iter().try_for_each(|f| check_ty(m, f.ty, seen)),
                AdtBody::Enum(variants) => variants
                    .iter()
                    .flat_map(|v| v.fields.iter())
                    .try_for_each(|f| check_ty(m, f.ty, seen)),
            }
        }
    }
}

/// The register type of a value: scalars by kind, every heap value a pointer.
pub(crate) fn clif_ty(m: &Module, ty: TyId) -> Result<Type, Unsupported> {
    Ok(match m.types.kind(ty) {
        TyKind::Unit | TyKind::Bool => types::I8,
        TyKind::Num(k) if k.is_float() => types::F64,
        TyKind::Num(_) => types::I64,
        _ => {
            check_ty(m, ty, &mut Vec::new())?;
            types::I64
        }
    })
}

pub(crate) fn is_heap(m: &Module, ty: TyId) -> bool {
    !matches!(
        m.types.kind(ty),
        TyKind::Unit | TyKind::Bool | TyKind::Num(_)
    )
}

pub(crate) fn is_scalar(m: &Module, ty: TyId) -> bool {
    !is_heap(m, ty)
}

/// How a list element is stored in memory.
#[derive(Clone, Copy)]
pub(crate) struct ElemLayout {
    /// Bytes per element.
    pub size: i64,
    /// Type of the memory access.
    pub mem: Type,
    /// Type of the register that holds the value.
    pub reg: Type,
    pub signed: bool,
    /// The element is an owned pointer.
    pub heap: bool,
}

pub(crate) fn elem_layout(m: &Module, elem: TyId) -> ElemLayout {
    let scalar = |size, mem, reg, signed| ElemLayout {
        size,
        mem,
        reg,
        signed,
        heap: false,
    };
    match m.types.kind(elem) {
        TyKind::Unit | TyKind::Bool => scalar(1, types::I8, types::I8, false),
        TyKind::Num(NumKind::U8) => scalar(1, types::I8, types::I64, false),
        TyKind::Num(NumKind::I32) => scalar(4, types::I32, types::I64, true),
        TyKind::Num(NumKind::U32) => scalar(4, types::I32, types::I64, false),
        TyKind::Num(NumKind::I64 | NumKind::U64) => scalar(8, types::I64, types::I64, true),
        TyKind::Num(_) => scalar(8, types::F64, types::F64, true),
        _ => ElemLayout {
            size: 8,
            mem: types::I64,
            reg: types::I64,
            signed: false,
            heap: true,
        },
    }
}

/// Bit `base + i` is set when `tys[i]` is a heap value.
pub(crate) fn ptr_mask(m: &Module, tys: &[TyId], base: usize) -> u64 {
    tys.iter()
        .enumerate()
        .filter(|(_, t)| is_heap(m, **t))
        .fold(0, |mask, (i, _)| mask | 1 << (base + i))
}

/// Slots of the object of `adt`, tag included for an enum.
pub(crate) fn adt_layout_slots(m: &Module, adt: AdtId) -> usize {
    layout::adt_layout(&m.types, adt).payload_slots as usize
}

// ---- compilation -----------------------------------------------------------

/// Functions reachable from `roots` through calls and closures, and which of
/// them are used as closure bodies (they need a uniform entry point).
fn reachable(m: &Module, roots: &[FuncId]) -> (Vec<bool>, Vec<bool>, bool) {
    let mut seen = vec![false; m.funcs.len()];
    let mut targets = vec![false; m.funcs.len()];
    let mut uses_globals = false;
    let mut stack: Vec<FuncId> = roots.to_vec();
    while let Some(id) = stack.pop() {
        if std::mem::replace(&mut seen[id.0 as usize], true) {
            continue;
        }
        for block in &m.func(id).blocks {
            for ins in &block.instrs {
                match ins {
                    Instr::Call { func, .. } => stack.push(*func),
                    Instr::Closure { func, .. } => {
                        targets[func.0 as usize] = true;
                        stack.push(*func);
                    }
                    Instr::GlobalGet { .. } | Instr::GlobalSet { .. } => uses_globals = true,
                    _ => {}
                }
            }
        }
    }
    (seen, targets, uses_globals)
}

struct ImportSpec {
    name: &'static str,
    ptr: *const u8,
    params: Vec<Type>,
    ret: Option<Type>,
}

fn import_table() -> Vec<ImportSpec> {
    use types::{F64, I32, I64, I8};
    let spec = |name, ptr: *const u8, params: &[Type], ret| ImportSpec {
        name,
        ptr,
        params: params.to_vec(),
        ret,
    };
    vec![
        spec("print", rt::print as *const u8, &[I64, I64, I64], None),
        spec("trap", rt::trap as *const u8, &[I64], None),
        spec("fmod", rt::fmod as *const u8, &[F64, F64], Some(F64)),
        spec("retain", rt::retain as *const u8, &[I64], None),
        spec("release", rt::release as *const u8, &[I64], None),
        spec(
            "list_new",
            rt::list_new as *const u8,
            &[I64, I64, I64],
            Some(I64),
        ),
        spec(
            "list_push",
            rt::list_push as *const u8,
            &[I64, I64],
            Some(I64),
        ),
        spec(
            "list_set",
            rt::list_set as *const u8,
            &[I64, I64, I64],
            Some(I64),
        ),
        spec("list_oob", rt::list_oob as *const u8, &[I64, I64], None),
        spec(
            "list_unique",
            rt::list_unique as *const u8,
            &[I64],
            Some(I64),
        ),
        spec(
            "list_take",
            rt::list_take as *const u8,
            &[I64, I64],
            Some(I64),
        ),
        spec("obj_new", rt::obj_new as *const u8, &[I64, I64], Some(I64)),
        spec("obj_unique", rt::obj_unique as *const u8, &[I64], Some(I64)),
        spec("map_new", rt::map_new as *const u8, &[I64], Some(I64)),
        spec("map_unique", rt::map_unique as *const u8, &[I64], Some(I64)),
        spec(
            "map_set",
            rt::map_set as *const u8,
            &[I64, I64, I64],
            Some(I64),
        ),
        spec("map_get", rt::map_get as *const u8, &[I64, I64], Some(I64)),
        spec(
            "map_take",
            rt::map_take as *const u8,
            &[I64, I64],
            Some(I64),
        ),
        spec(
            "map_get_opt",
            rt::map_get_opt as *const u8,
            &[I64, I64, I64],
            Some(I64),
        ),
        spec(
            "list_pop_opt",
            rt::list_pop_opt as *const u8,
            &[I64, I64],
            Some(I64),
        ),
        spec("map_len", rt::map_len as *const u8, &[I64], Some(I64)),
        spec("map_has", rt::map_has as *const u8, &[I64, I64], Some(I8)),
        spec(
            "str_concat",
            rt::str_concat as *const u8,
            &[I64, I64],
            Some(I64),
        ),
        spec("str_len", rt::str_len as *const u8, &[I64], Some(I64)),
        spec("str_cmp", rt::str_cmp as *const u8, &[I64, I64], Some(I64)),
        spec("to_str", rt::rt_to_str as *const u8, &[I32, I64], Some(I64)),
        spec(
            "cmp",
            rt::rt_cmp as *const u8,
            &[I32, I32, I64, I64],
            Some(I8),
        ),
        spec(
            "cast",
            rt::rt_cast as *const u8,
            &[I32, I32, I64],
            Some(I64),
        ),
        spec(
            "call",
            rt::rt_call as *const u8,
            &[I64, I64, I64, I64, I32],
            Some(I64),
        ),
    ]
}
