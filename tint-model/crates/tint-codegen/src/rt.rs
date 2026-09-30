//! Runtime the generated code calls into.
//!
//! Every heap value is a pointer to an object that starts with a
//! [`Header`]: a reference count and a kind. The generated code reads the
//! fast-path fields (`rc`, list `len`/`cap`/`ptr`, object slots) directly and
//! only calls in here to allocate, free, grow, copy a shared object before a
//! write, or fail.
//!
//! - `List`: `len`, `cap`, `ptr` to `esize`-byte elements. Scalar elements are
//!   stored raw; heap elements are 8-byte owned pointers (`heap`).
//! - `Obj` (struct, tuple, enum): `nslots` 8-byte slots. `mask` says which
//!   slots hold owned pointers; an enum keeps its variant tag in slot 0.
//! - `Str`: an immutable `String`.
//!
//! Calls the backend does not implement itself go through `rt_call`, which
//! converts the operands to interpreter values and runs the reference
//! implementation, so they mean exactly what they mean there.

use crate::{elem_layout, is_heap, ptr_mask};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::BTreeMap;
use tint_ir::typed::interp::{cast_num, display, render, AdtVal, ClosureVal, Interp};
use tint_ir::typed::*;

const KIND_LIST: usize = 1;
const KIND_OBJ: usize = 2;
const KIND_STR: usize = 3;
const KIND_MAP: usize = 4;

/// Reference count of objects that are never freed (string literals).
const IMMORTAL: usize = 1 << 40;

#[repr(C)]
pub struct Header {
    rc: usize,
    kind: usize,
}

#[repr(C)]
pub struct List {
    head: Header,
    len: usize,
    cap: usize,
    ptr: *mut u8,
    esize: usize,
    heap: usize,
    buf: Vec<u8>,
}

/// A map from string keys to values; `heap` says the values are owned pointers.
#[repr(C)]
pub struct Map {
    head: Header,
    heap: usize,
    map: BTreeMap<String, u64>,
}

#[repr(C)]
pub struct Str {
    head: Header,
    s: String,
}

pub const OFF_RC: i32 = 0;
pub const OFF_LEN: i32 = 16;
pub const OFF_CAP: i32 = 24;
pub const OFF_PTR: i32 = 32;
/// Offset of slot 0 of an `Obj`.
pub const OFF_SLOTS: i32 = 32;

type Ptr = *mut Header;

// ---- context ---------------------------------------------------------------

struct Ctx {
    module: *const Module,
    msgs: Vec<String>,
    interp: Option<Box<Interp<'static>>>,
}

thread_local! {
    static CTX: RefCell<Ctx> = const {
        RefCell::new(Ctx { module: std::ptr::null(), msgs: Vec::new(), interp: None })
    };
}

pub fn enter(module: &Module, msgs: &[String]) {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.module = module as *const Module;
        c.msgs = msgs.to_vec();
        // SAFETY: the module outlives every call made from generated code.
        let m: &'static Module = unsafe { &*(module as *const Module) };
        c.interp = Some(Box::new(Interp::new(m)));
    });
}

fn module() -> &'static Module {
    CTX.with(|c| unsafe { &*c.borrow().module })
}

fn fail(msg: String) -> ! {
    eprintln!("trap: {msg}");
    std::process::exit(1);
}

pub extern "C" fn trap(msg: i64) {
    let text = CTX.with(|c| c.borrow().msgs.get(msg as usize).cloned().unwrap_or_default());
    fail(text);
}

pub extern "C" fn fmod(a: f64, b: f64) -> f64 {
    a % b
}

// ---- reference counting ----------------------------------------------------

pub extern "C" fn retain(p: Ptr) {
    if !p.is_null() {
        unsafe { (*p).rc += 1 };
    }
}

pub extern "C" fn release(p: Ptr) {
    if p.is_null() {
        return;
    }
    unsafe {
        (*p).rc -= 1;
        if (*p).rc == 0 {
            free(p);
        }
    }
}

unsafe fn free(p: Ptr) {
    match (*p).kind {
        KIND_LIST => {
            let list = Box::from_raw(p as *mut List);
            if list.heap != 0 {
                for i in 0..list.len {
                    release(*(list.ptr.add(i * 8) as *mut Ptr));
                }
            }
        }
        KIND_OBJ => {
            let n = *(p as *const usize).add(2);
            let mask = *(p as *const u64).add(3);
            for i in 0..n {
                if mask >> i & 1 == 1 {
                    release(*slot_ptr(p, i));
                }
            }
            dealloc(p as *mut u8, obj_layout(n));
        }
        KIND_STR => drop(Box::from_raw(p as *mut Str)),
        KIND_MAP => {
            let map = Box::from_raw(p as *mut Map);
            if map.heap != 0 {
                for v in map.map.values() {
                    release(*v as Ptr);
                }
            }
        }
        _ => {}
    }
}

// ---- objects (struct, tuple, enum) -----------------------------------------

fn obj_layout(nslots: usize) -> Layout {
    Layout::from_size_align(OFF_SLOTS as usize + 8 * nslots, 8).unwrap()
}

unsafe fn slot_ptr(p: Ptr, i: usize) -> *mut Ptr {
    (p as *mut u8).add(OFF_SLOTS as usize + 8 * i) as *mut Ptr
}

pub extern "C" fn obj_new(nslots: i64, mask: u64) -> Ptr {
    unsafe {
        let p = alloc_zeroed(obj_layout(nslots as usize)) as Ptr;
        (*p).rc = 1;
        (*p).kind = KIND_OBJ;
        let words = p as *mut usize;
        *words.add(2) = nslots as usize;
        *words.add(3) = mask as usize;
        p
    }
}

/// A copy of the object that is safe to write to: `p` itself when unshared.
pub static COPIES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub extern "C" fn obj_unique(p: Ptr) -> Ptr {
    unsafe {
        if (*p).rc == 1 {
            return p;
        }
        COPIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let n = *(p as *const usize).add(2);
        let mask = *(p as *const u64).add(3);
        let copy = obj_new(n as i64, mask);
        for i in 0..n {
            let v = *slot_ptr(p, i);
            *slot_ptr(copy, i) = v;
            if mask >> i & 1 == 1 {
                retain(v);
            }
        }
        release(p);
        copy
    }
}

/// An object of an enum variant without fields, allocated once and never freed.
pub fn immortal_variant(nslots: usize, tag: u64) -> Ptr {
    let p = obj_new(nslots as i64, 0);
    unsafe {
        (*p).rc = IMMORTAL;
        *(slot_ptr(p, 0) as *mut u64) = tag;
    }
    p
}

// ---- strings ---------------------------------------------------------------

fn new_str(s: String) -> Ptr {
    Box::into_raw(Box::new(Str { head: Header { rc: 1, kind: KIND_STR }, s })) as Ptr
}

/// A string literal: allocated once at compile time, never freed.
pub fn immortal_str(s: &str) -> Ptr {
    let p = new_str(s.to_string());
    unsafe { (*p).rc = IMMORTAL };
    p
}

unsafe fn str_of<'a>(p: Ptr) -> &'a str {
    &(*(p as *const Str)).s
}

pub extern "C" fn str_concat(n: i64, parts: *const Ptr) -> Ptr {
    let mut out = String::new();
    for i in 0..n as usize {
        out.push_str(unsafe { str_of(*parts.add(i)) });
    }
    new_str(out)
}

pub extern "C" fn str_len(p: Ptr) -> i64 {
    unsafe { str_of(p).chars().count() as i64 }
}

/// -1, 0 or 1, comparing bytes like the interpreter does.
pub extern "C" fn str_cmp(a: Ptr, b: Ptr) -> i64 {
    unsafe { str_of(a).cmp(str_of(b)) as i64 }
}

// ---- maps ------------------------------------------------------------------

pub extern "C" fn map_new(heap: i64) -> Ptr {
    Box::into_raw(Box::new(Map {
        head: Header { rc: 1, kind: KIND_MAP },
        heap: heap as usize,
        map: BTreeMap::new(),
    })) as Ptr
}

/// A map this caller may write to: `p` itself when unshared, else a copy.
pub extern "C" fn map_unique(p: Ptr) -> Ptr {
    unsafe {
        if (*p).rc == 1 {
            return p;
        }
        let old = &*(p as *const Map);
        if old.heap != 0 {
            for v in old.map.values() {
                retain(*v as Ptr);
            }
        }
        let copy = Map {
            head: Header { rc: 1, kind: KIND_MAP },
            heap: old.heap,
            map: old.map.clone(),
        };
        release(p);
        Box::into_raw(Box::new(copy)) as Ptr
    }
}

/// Inserts or replaces an entry. A heap value's reference moves into the map.
pub extern "C" fn map_set(p: Ptr, key: Ptr, bits: u64) -> Ptr {
    unsafe {
        let p = map_unique(p);
        let map = &mut *(p as *mut Map);
        let old = map.map.insert(str_of(key).to_string(), bits);
        if let (Some(old), true) = (old, map.heap != 0) {
            release(old as Ptr);
        }
        p
    }
}

pub extern "C" fn map_get(p: Ptr, key: Ptr) -> u64 {
    unsafe {
        let map = &*(p as *const Map);
        match map.map.get(str_of(key)) {
            Some(v) => *v,
            None => fail(format!("key not found: {:?}", str_of(key))),
        }
    }
}

/// Moves a heap value out of a map this caller already owns uniquely.
pub extern "C" fn map_take(p: Ptr, key: Ptr) -> u64 {
    unsafe {
        let map = &mut *(p as *mut Map);
        match map.map.get_mut(str_of(key)) {
            Some(v) => std::mem::replace(v, 0),
            None => fail(format!("key not found: {:?}", str_of(key))),
        }
    }
}

/// `Option<V>` object: `Some(value)` when `found`, else `None`. A heap value
/// is retained for the new object.
unsafe fn option_obj(found: Option<u64>, heap: bool, layout: &[i64; 5]) -> Ptr {
    let [some_tag, none_tag, nslots, mask, _] = *layout;
    match found {
        Some(bits) => {
            let o = obj_new(nslots, mask as u64);
            *(slot_ptr(o, 0) as *mut u64) = some_tag as u64;
            *(slot_ptr(o, 1) as *mut u64) = bits;
            if heap {
                retain(bits as Ptr);
            }
            o
        }
        None => {
            let o = obj_new(nslots, 0);
            *(slot_ptr(o, 0) as *mut u64) = none_tag as u64;
            o
        }
    }
}

/// `map.get(key)`: the `Option` object described by `layout`
/// (`[some tag, none tag, slots, pointer mask, payload is a signed 32-bit int]`).
pub extern "C" fn map_get_opt(p: Ptr, key: Ptr, layout: *const [i64; 5]) -> Ptr {
    unsafe {
        let map = &*(p as *const Map);
        option_obj(map.map.get(str_of(key)).copied(), map.heap != 0, &*layout)
    }
}

pub extern "C" fn map_len(p: Ptr) -> i64 {
    unsafe { (*(p as *const Map)).map.len() as i64 }
}

pub extern "C" fn map_has(p: Ptr, key: Ptr) -> u8 {
    unsafe { (*(p as *const Map)).map.contains_key(str_of(key)) as u8 }
}

// ---- lists -----------------------------------------------------------------

fn new_list(cap: usize, esize: usize, heap: bool) -> List {
    let mut buf: Vec<u8> = Vec::with_capacity(cap * esize);
    List {
        head: Header { rc: 1, kind: KIND_LIST },
        len: 0,
        cap: buf.capacity() / esize,
        ptr: buf.as_mut_ptr(),
        esize,
        heap: heap as usize,
        buf,
    }
}

/// Makes the owned `Vec` agree with the element count the generated code
/// maintains without telling it.
fn sync(l: &mut List) {
    unsafe { l.buf.set_len(l.len * l.esize) };
}

pub extern "C" fn list_new(cap: i64, esize: i64, heap: i64) -> Ptr {
    Box::into_raw(Box::new(new_list(cap.max(0) as usize, esize as usize, heap != 0))) as Ptr
}

/// A list this caller may write to: `p` itself when unshared, otherwise a
/// private copy (and the reference to `p` is released).
pub extern "C" fn list_unique(p: Ptr) -> Ptr {
    unsafe {
        if (*p).rc == 1 {
            return p;
        }
        let old = &mut *(p as *mut List);
        sync(old);
        let mut copy = new_list(old.len.max(1), old.esize, old.heap != 0);
        copy.buf.extend_from_slice(&old.buf);
        copy.len = old.len;
        copy.ptr = copy.buf.as_mut_ptr();
        if old.heap != 0 {
            for i in 0..old.len {
                retain(*(old.ptr.add(i * 8) as *mut Ptr));
            }
        }
        release(p);
        Box::into_raw(Box::new(copy)) as Ptr
    }
}

unsafe fn write_elem(l: &mut List, index: usize, bits: u64) {
    let src = bits.to_le_bytes();
    std::ptr::copy_nonoverlapping(src.as_ptr(), l.ptr.add(index * l.esize), l.esize);
}

/// Slow path of `push`: the list is shared or full. A heap element's
/// reference moves into the list.
pub extern "C" fn list_push(p: Ptr, bits: u64) -> Ptr {
    unsafe {
        let p = list_unique(p);
        let l = &mut *(p as *mut List);
        sync(l);
        if l.len == l.cap {
            l.buf.reserve(l.cap.max(4) * l.esize);
            l.ptr = l.buf.as_mut_ptr();
            l.cap = l.buf.capacity() / l.esize;
        }
        write_elem(l, l.len, bits);
        l.len += 1;
        p
    }
}

fn oob(index: i64, len: usize) -> ! {
    fail(format!("index {index} out of bounds (len={len})"))
}

/// Slow path of `xs[i] = v`: the list is shared, the index is bad, or the
/// elements are heap values (whose old value is released here).
pub extern "C" fn list_set(p: Ptr, index: i64, bits: u64) -> Ptr {
    unsafe {
        let len = (*(p as *mut List)).len;
        if index < 0 || index as usize >= len {
            oob(index, len);
        }
        let p = list_unique(p);
        let l = &mut *(p as *mut List);
        if l.heap != 0 {
            release(*(l.ptr.add(index as usize * 8) as *mut Ptr));
        }
        write_elem(l, index as usize, bits);
        p
    }
}

/// Moves a heap element out of a list this caller already owns uniquely.
pub extern "C" fn list_take(p: Ptr, index: i64) -> u64 {
    unsafe {
        let l = &mut *(p as *mut List);
        if index < 0 || index as usize >= l.len {
            oob(index, l.len);
        }
        let slot = l.ptr.add(index as usize * 8) as *mut u64;
        let v = *slot;
        *slot = 0;
        v
    }
}

/// `list.pop()` on a list this caller already owns uniquely: the removed
/// element's reference moves into the returned `Option` object.
pub extern "C" fn list_pop_opt(p: Ptr, layout: *const [i64; 5]) -> Ptr {
    unsafe {
        let l = &mut *(p as *mut List);
        if l.len == 0 {
            return option_obj(None, false, &*layout);
        }
        l.len -= 1;
        let at = l.ptr.add(l.len * l.esize);
        let bits = if l.heap != 0 {
            *(at as *const u64)
        } else {
            let mut raw = [0u8; 8];
            std::ptr::copy_nonoverlapping(at, raw.as_mut_ptr(), l.esize);
            u64::from_le_bytes(raw)
        };
        // Scalars keep the same bit convention as registers; narrow signed
        // elements need sign extension.
        let bits = if l.heap == 0 && l.esize == 4 && (*layout)[4] != 0 {
            bits as u32 as i32 as i64 as u64
        } else {
            bits
        };
        option_obj(Some(bits), false, &*layout)
    }
}

pub extern "C" fn list_oob(p: Ptr, index: i64) {
    oob(index, unsafe { (*(p as *mut List)).len });
}

// ---- values <-> heap objects ------------------------------------------------

fn scalar_val(kind: NumKind, bits: u64) -> Val {
    if kind.is_float() {
        Val::Float(f64::from_bits(bits))
    } else {
        Val::Int(bits as i64)
    }
}

/// Reads the elements of the list `p` (elements of type `elem`).
unsafe fn list_to_val(m: &Module, elem: TyId, p: Ptr) -> Val {
    let l = &*(p as *const List);
    let layout = elem_layout(m, elem);
    let mut items = Vec::with_capacity(l.len);
    for i in 0..l.len {
        let at = l.ptr.add(i * l.esize);
        let bits = if layout.heap {
            *(at as *const u64)
        } else {
            let mut raw = [0u8; 8];
            std::ptr::copy_nonoverlapping(at, raw.as_mut_ptr(), l.esize);
            let v = u64::from_le_bytes(raw);
            if layout.signed && l.esize == 4 {
                v as u32 as i32 as i64 as u64
            } else {
                v
            }
        };
        items.push(to_val(m, elem, bits));
    }
    Val::list(items)
}

/// An interpreter value for the register value `bits` of type `ty`
/// (the object stays owned by the caller).
pub fn to_val(m: &Module, ty: TyId, bits: u64) -> Val {
    let p = bits as Ptr;
    unsafe {
        match m.types.kind(ty) {
            TyKind::Unit => Val::Unit,
            TyKind::Bool => Val::Bool(bits & 0xff != 0),
            TyKind::Num(k) => scalar_val(*k, bits),
            TyKind::Str => Val::str(str_of(p)),
            TyKind::List(e) => list_to_val(m, *e, p),
            TyKind::Tuple(items) => {
                let vals = items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| to_val(m, *t, *(slot_ptr(p, i) as *const u64)))
                    .collect();
                Val::Tuple(Rc::new(vals))
            }
            TyKind::Adt(adt) => {
                let def = m.types.adt(*adt);
                let (tag, fields) = match &def.body {
                    AdtBody::Struct(fields) => (0, fields.iter().map(|f| f.ty).collect::<Vec<_>>()),
                    AdtBody::Enum(variants) => {
                        let tag = *(slot_ptr(p, 0) as *const u64) as usize;
                        (tag as u32, variants[tag].fields.iter().map(|f| f.ty).collect())
                    }
                };
                let base = if matches!(def.body, AdtBody::Enum(_)) { 1 } else { 0 };
                let vals = fields
                    .iter()
                    .enumerate()
                    .map(|(i, t)| to_val(m, *t, *(slot_ptr(p, base + i) as *const u64)))
                    .collect();
                Val::Adt(Rc::new(AdtVal { adt: *adt, tag, fields: vals }))
            }
            TyKind::Map(v) => {
                let map = &*(p as *const Map);
                let entries: BTreeMap<String, Val> =
                    map.map.iter().map(|(k, bits)| (k.clone(), to_val(m, *v, *bits))).collect();
                Val::Map(Rc::new(entries))
            }
            TyKind::Fn(..) => {
                let id = FuncId(*(slot_ptr(p, 0) as *const u64) as u32);
                let f = m.func(id);
                let captures = (0..f.ncaptures as usize)
                    .map(|i| to_val(m, f.reg_ty(f.params[i]), *(slot_ptr(p, 1 + i) as *const u64)))
                    .collect();
                Val::Closure(Rc::new(ClosureVal { func: id, captures }))
            }
        }
    }
}

fn float_bits(v: &Val) -> u64 {
    match v {
        Val::Float(f) => f.to_bits(),
        Val::Int(i) => *i as u64,
        Val::Bool(b) => *b as u64,
        _ => 0,
    }
}

/// A fresh object (one owned reference) or scalar bits for `v` of type `ty`.
pub fn from_val(m: &Module, ty: TyId, v: &Val) -> u64 {
    match (m.types.kind(ty), v) {
        (TyKind::Unit, _) => 0,
        (TyKind::Bool | TyKind::Num(_), v) => float_bits(v),
        (TyKind::Str, Val::Str(s)) => new_str(s.to_string()) as u64,
        (TyKind::List(e), Val::List(items)) => {
            let layout = elem_layout(m, *e);
            let mut p = list_new(items.len() as i64, layout.size, layout.heap as i64);
            for item in items.iter() {
                p = list_push(p, from_val(m, *e, item));
            }
            p as u64
        }
        (TyKind::Tuple(tys), Val::Tuple(items)) => {
            let mask = ptr_mask(m, tys, 0);
            let p = obj_new(tys.len() as i64, mask);
            for (i, (t, item)) in tys.iter().zip(items.iter()).enumerate() {
                unsafe { *(slot_ptr(p, i) as *mut u64) = from_val(m, *t, item) };
            }
            p as u64
        }
        (TyKind::Adt(adt), Val::Adt(a)) => {
            let def = m.types.adt(*adt);
            let (tys, base): (Vec<TyId>, usize) = match &def.body {
                AdtBody::Struct(fields) => (fields.iter().map(|f| f.ty).collect(), 0),
                AdtBody::Enum(variants) => {
                    (variants[a.tag as usize].fields.iter().map(|f| f.ty).collect(), 1)
                }
            };
            let nslots = crate::adt_layout_slots(m, *adt);
            let p = obj_new(nslots as i64, ptr_mask(m, &tys, base));
            unsafe {
                if base == 1 {
                    *(slot_ptr(p, 0) as *mut u64) = a.tag as u64;
                }
                for (i, (t, item)) in tys.iter().zip(a.fields.iter()).enumerate() {
                    *(slot_ptr(p, base + i) as *mut u64) = from_val(m, *t, item);
                }
            }
            p as u64
        }
        (TyKind::Map(e), Val::Map(entries)) => {
            let mut p = map_new(is_heap(m, *e) as i64);
            for (k, item) in entries.iter() {
                let key = new_str(k.clone());
                p = map_set(p, key, from_val(m, *e, item));
                release(key);
            }
            p as u64
        }
        (TyKind::Fn(..), Val::Closure(c)) => {
            let f = m.func(c.func);
            let tys: Vec<TyId> = (0..f.ncaptures as usize).map(|i| f.reg_ty(f.params[i])).collect();
            let p = obj_new(1 + tys.len() as i64, ptr_mask(m, &tys, 1));
            unsafe {
                *(slot_ptr(p, 0) as *mut u64) = c.func.0 as u64;
                for (i, (t, item)) in tys.iter().zip(c.captures.iter()).enumerate() {
                    *(slot_ptr(p, 1 + i) as *mut u64) = from_val(m, *t, item);
                }
            }
            p as u64
        }
        (other, v) => fail(format!("internal error: cannot build {other:?} from {v:?}")),
    }
}

// ---- bridge to the reference implementation ---------------------------------

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
];

pub fn rt_fn_index(f: RtFn) -> i64 {
    RT_FNS.iter().position(|x| *x == f).expect("listed") as i64
}

/// Runs a runtime call on the interpreter. `args` holds the operands (borrowed);
/// for a call that mutates its first operand, `args[0]` receives the new
/// object (one owned reference) that replaces it.
pub extern "C" fn rt_call(f: i64, n: i64, args: *mut u64, tys: *const u32, dst_ty: u32) -> u64 {
    let m = module();
    let n = n as usize;
    let f = RT_FNS[f as usize];
    let arg_tys: Vec<TyId> = (0..n).map(|i| TyId(unsafe { *tys.add(i) })).collect();
    let vals: Vec<Val> = (0..n)
        .map(|i| to_val(m, arg_tys[i], unsafe { *args.add(i) }))
        .collect();
    let dst_ty = TyId(dst_ty);
    let outcome = CTX.with(|c| {
        let mut c = c.borrow_mut();
        c.interp.as_mut().expect("entered").rt_values(f, &arg_tys, dst_ty, vals)
    });
    match outcome {
        Ok((result, after)) => {
            if f.mutates_first() {
                unsafe { *args = from_val(m, arg_tys[0], &after[0]) };
            }
            from_val(m, dst_ty, &result)
        }
        Err(trap) => fail(trap.msg),
    }
}

/// `a <op> b` on two values of type `ty`.
pub extern "C" fn rt_cmp(ty: u32, op: u32, a: u64, b: u64) -> u8 {
    let m = module();
    let ty = TyId(ty);
    let ops = [CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge];
    let (va, vb) = (to_val(m, ty, a), to_val(m, ty, b));
    let outcome = CTX.with(|c| c.borrow().interp.as_ref().expect("entered").compare_values(ty, ops[op as usize], &va, &vb));
    match outcome {
        Ok(r) => r as u8,
        Err(trap) => fail(trap.msg),
    }
}

pub const NUM_KINDS: [NumKind; 8] = [
    NumKind::Num,
    NumKind::I32,
    NumKind::I64,
    NumKind::U8,
    NumKind::U32,
    NumKind::U64,
    NumKind::F32,
    NumKind::F64,
];

pub fn num_kind_index(k: NumKind) -> i64 {
    NUM_KINDS.iter().position(|x| *x == k).unwrap() as i64
}

/// A checked numeric conversion (`as`).
pub extern "C" fn rt_cast(from: u32, to: u32, bits: u64) -> u64 {
    let (from, to) = (NUM_KINDS[from as usize], NUM_KINDS[to as usize]);
    match cast_num(from, to, &scalar_val(from, bits)) {
        Ok(v) => float_bits(&v),
        Err(trap) => fail(trap.msg),
    }
}

/// The text `"{value}"` produces.
pub extern "C" fn rt_to_str(ty: u32, bits: u64) -> Ptr {
    let m = module();
    let ty = TyId(ty);
    new_str(display(m, ty, &to_val(m, ty, bits)))
}

pub extern "C" fn print(ty: i64, bits: i64, newline: i64) {
    use std::io::Write;
    let m = module();
    let ty = TyId(ty as u32);
    let text = display(m, ty, &to_val(m, ty, bits as u64));
    let mut out = std::io::stdout().lock();
    let _ = if newline != 0 { writeln!(out, "{text}") } else { write!(out, "{text}") };
}

/// The result of `main`, rendered like the interpreter renders it.
pub fn render_bits(m: &Module, ty: TyId, bits: u64) -> String {
    let text = render(m, ty, &to_val(m, ty, bits));
    if is_heap(m, ty) {
        release(bits as Ptr);
    }
    text
}
