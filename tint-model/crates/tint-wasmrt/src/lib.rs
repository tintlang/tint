//! Runtime of Tint programs compiled to WebAssembly (`tint-wasmgen`).
//!
//! A wasm32 module that owns the linear memory; the generated `app` module
//! imports its memory and functions. Object layout is described in
//! `tint-wasmabi`. Operations the generated code does not do inline (allocate,
//! free, grow, copy a shared object before a write, strings, maps) live
//! here; operations on many types at once (`to_str`, equality of aggregates,
//! `sort`, ...) convert the operands to interpreter values and run the
//! reference implementation, so they mean exactly what they mean there.
//!
//! The host provides `env.host_trap(ptr, len)` and `env.host_print(ptr, len,
//! newline)`.
#![cfg(target_arch = "wasm32")]
#![allow(clippy::missing_safety_doc)]

use std::alloc::{alloc_zeroed, dealloc, realloc, Layout};
use std::collections::BTreeMap;
use tint_ir::typed::interp::{cast_num, display, render, Interp};
use tint_ir::typed::*;
use tint_wasmabi::*;

use values::{from_val, scalar_val, to_val};

mod host;
mod values;

pub use host::{
    host_storage_hydrate, host_storage_snapshot, host_take_http, new_string_ptr, release_ptr,
    set_native_hook, take_callback, NativeReply,
};

#[cfg(not(feature = "dom"))]
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_trap(ptr: *const u8, len: u32);
    fn host_print(ptr: *const u8, len: u32, newline: u32);
    fn host_now_ms() -> f64;
}

#[cfg(feature = "dom")]
unsafe fn host_trap(ptr: *const u8, len: u32) {
    let text = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len as usize)).into_owned();
    wasm_bindgen::throw_str(&text)
}

#[cfg(feature = "dom")]
unsafe fn host_print(ptr: *const u8, len: u32, newline: u32) {
    let text = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len as usize)).into_owned();
    let _ = newline;
    web_sys::console::log_1(&text.into());
}

#[cfg(feature = "dom")]
unsafe fn host_now_ms() -> f64 {
    js_sys::Date::now()
}

#[repr(C)]
pub struct Header {
    rc: u32,
    kind: u32,
}

#[repr(C)]
pub(crate) struct List {
    head: Header,
    len: u32,
    cap: u32,
    ptr: *mut u8,
    esize: u32,
    heap: u32,
}

#[repr(C)]
pub(crate) struct Map {
    head: Header,
    heap: u32,
    map: BTreeMap<String, u64>,
}

#[repr(C)]
pub(crate) struct Str {
    head: Header,
    s: String,
}

pub(crate) type Ptr = *mut Header;

const _: () = {
    assert!(std::mem::size_of::<Header>() == 8);
    assert!(std::mem::offset_of!(List, len) as u64 == OFF_LEN);
    assert!(std::mem::offset_of!(List, cap) as u64 == OFF_CAP);
    assert!(std::mem::offset_of!(List, ptr) as u64 == OFF_PTR);
    assert!(std::mem::offset_of!(List, esize) as u64 == OFF_ESIZE);
    assert!(std::mem::offset_of!(List, heap) as u64 == OFF_HEAP);
};

#[cfg(feature = "ui")]
pub mod ui;

// ---- context -----------------------------------------------------------------

struct State {
    module: &'static Module,
    msgs: Vec<String>,
    meta: String,
    interp: Interp<'static>,
    scratch: Vec<u64>,
}

static mut STATE: *mut State = std::ptr::null_mut();

/// Heap objects allocated and not freed yet (for leak tests).
static mut LIVE: i64 = 0;

#[no_mangle]
pub extern "C" fn rt_live() -> i64 {
    unsafe { LIVE }
}

fn born() {
    unsafe { LIVE += 1 };
}

fn state() -> &'static mut State {
    unsafe {
        if STATE.is_null() {
            host_fail("runtime used before rt_init");
        }
        &mut *STATE
    }
}

pub(crate) fn module() -> &'static Module {
    state().module
}

pub(crate) fn host_fail(msg: &str) -> ! {
    unsafe { host_trap(msg.as_ptr(), msg.len() as u32) };
    core::arch::wasm32::unreachable()
}

pub(crate) fn fail(msg: String) -> ! {
    host_fail(&msg)
}

#[no_mangle]
pub extern "C" fn rt_alloc(n: u32) -> *mut u8 {
    unsafe { alloc_zeroed(Layout::from_size_align(n.max(1) as usize, 8).unwrap()) }
}

/// Installs the module descriptor (see `tint_wasmabi::encode`).
#[no_mangle]
pub unsafe extern "C" fn rt_init(ptr: *const u8, len: u32) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let (module, msgs, meta) = decode(bytes);
    let module: &'static Module = Box::leak(Box::new(module));
    let state = Box::new(State {
        module,
        msgs,
        meta,
        interp: Interp::new(module),
        scratch: vec![0; 64],
    });
    STATE = Box::into_raw(state);
}

/// Parameter kinds of the app's exports: `(name, kinds)`, see `Module::export_sigs`.
pub fn export_sig(name: &str) -> Option<String> {
    module()
        .export_sigs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, k)| k.clone())
}

/// A scratch buffer of at least `n` bytes, valid until the next call.
#[no_mangle]
pub extern "C" fn rt_scratch(n: u32) -> *mut u8 {
    let s = state();
    let words = (n as usize).div_ceil(8).max(1);
    if s.scratch.len() < words {
        s.scratch.resize(words, 0);
    }
    s.scratch.as_mut_ptr() as *mut u8
}

#[no_mangle]
pub extern "C" fn trap(id: u32) {
    let msg = state().msgs.get(id as usize).cloned().unwrap_or_default();
    fail(msg)
}

#[no_mangle]
pub extern "C" fn tint_fmod(a: f64, b: f64) -> f64 {
    a % b
}

/// A numeric conversion that failed its check: reports the interpreter's message.
#[no_mangle]
pub extern "C" fn cast_fail(from: u32, to: u32, bits: u64) {
    let (from, to) = (KINDS[from as usize], KINDS[to as usize]);
    match cast_num(from, to, &scalar_val(from, bits)) {
        Err(t) => fail(t.msg),
        Ok(_) => fail("numeric cast failed".into()),
    }
}

// ---- reference counting ---------------------------------------------------------

#[no_mangle]
pub extern "C" fn retain(p: Ptr) {
    if !p.is_null() {
        unsafe { (*p).rc += 1 };
    }
}

#[no_mangle]
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
    LIVE -= 1;
    match (*p).kind {
        KIND_LIST => {
            let list = Box::from_raw(p as *mut List);
            if list.heap != 0 {
                for i in 0..list.len as usize {
                    release(*(list.ptr.add(i * 8) as *mut u64) as usize as Ptr);
                }
            }
            if list.cap > 0 {
                dealloc(list.ptr, buf_layout(list.cap, list.esize));
            }
        }
        KIND_OBJ => {
            let n = *(p as *const u32).add(2);
            let mask = *(p as *const u64).add(2);
            for i in 0..n {
                if mask >> i & 1 == 1 {
                    release(slot_ptr(p, i as usize).read() as usize as Ptr);
                }
            }
            dealloc(p as *mut u8, obj_layout(n));
        }
        KIND_STR => drop(Box::from_raw(p as *mut Str)),
        KIND_MAP => {
            let map = Box::from_raw(p as *mut Map);
            if map.heap != 0 {
                for v in map.map.values() {
                    release(*v as usize as Ptr);
                }
            }
        }
        _ => {}
    }
}

// ---- objects (struct, tuple, enum, closure) ---------------------------------------------

fn obj_layout(nslots: u32) -> Layout {
    Layout::from_size_align(OFF_SLOTS as usize + 8 * nslots as usize, 8).unwrap()
}

pub(crate) unsafe fn slot_ptr(p: Ptr, i: usize) -> *mut u64 {
    (p as *mut u8).add(OFF_SLOTS as usize + 8 * i) as *mut u64
}

#[no_mangle]
pub extern "C" fn obj_new(nslots: u32, mask: u64) -> Ptr {
    unsafe {
        born();
        let p = alloc_zeroed(obj_layout(nslots)) as Ptr;
        (*p).rc = 1;
        (*p).kind = KIND_OBJ;
        *(p as *mut u32).add(2) = nslots;
        *(p as *mut u64).add(2) = mask;
        p
    }
}

/// A copy of the object that is safe to write to: `p` itself when unshared.
#[no_mangle]
pub extern "C" fn obj_unique(p: Ptr) -> Ptr {
    unsafe {
        if (*p).rc == 1 {
            return p;
        }
        let n = *(p as *const u32).add(2);
        let mask = *(p as *const u64).add(2);
        let copy = obj_new(n, mask);
        for i in 0..n as usize {
            let v = *slot_ptr(p, i);
            *slot_ptr(copy, i) = v;
            if mask >> i & 1 == 1 {
                retain(v as usize as Ptr);
            }
        }
        release(p);
        copy
    }
}

/// An object of an enum variant without fields, allocated once and never freed.
#[no_mangle]
pub extern "C" fn immortal_variant(nslots: u32, tag: u32) -> Ptr {
    let p = obj_new(nslots, 0);
    unsafe {
        (*p).rc = IMMORTAL;
        *slot_ptr(p, 0) = tag as u64;
    }
    p
}

// ---- strings -----------------------------------------------------------------------

pub(crate) fn new_str(s: String) -> Ptr {
    born();
    Box::into_raw(Box::new(Str {
        head: Header {
            rc: 1,
            kind: KIND_STR,
        },
        s,
    })) as Ptr
}

/// A string literal, allocated once and never freed.
#[no_mangle]
pub unsafe extern "C" fn str_lit(ptr: *const u8, len: u32) -> Ptr {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let p = new_str(String::from_utf8_lossy(bytes).into_owned());
    (*p).rc = IMMORTAL;
    p
}

pub(crate) unsafe fn str_of<'a>(p: Ptr) -> &'a str {
    &(*(p as *const Str)).s
}

#[no_mangle]
pub unsafe extern "C" fn str_data(p: Ptr) -> *const u8 {
    str_of(p).as_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn str_bytes(p: Ptr) -> u32 {
    str_of(p).len() as u32
}

/// `parts` points at `n` consecutive `u32` string pointers.
#[no_mangle]
pub unsafe extern "C" fn str_concat(n: u32, parts: *const u32) -> Ptr {
    let mut out = String::new();
    for i in 0..n as usize {
        out.push_str(str_of(*parts.add(i) as usize as Ptr));
    }
    new_str(out)
}

#[no_mangle]
pub unsafe extern "C" fn str_len(p: Ptr) -> u32 {
    str_of(p).chars().count() as u32
}

/// -1, 0 or 1, comparing bytes like the interpreter does.
#[no_mangle]
pub unsafe extern "C" fn str_cmp(a: Ptr, b: Ptr) -> i32 {
    str_of(a).cmp(str_of(b)) as i32
}

// ---- maps ----------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn map_new(heap: u32) -> Ptr {
    born();
    Box::into_raw(Box::new(Map {
        head: Header {
            rc: 1,
            kind: KIND_MAP,
        },
        heap,
        map: BTreeMap::new(),
    })) as Ptr
}

/// A map this caller may write to: `p` itself when unshared, else a copy.
#[no_mangle]
pub unsafe extern "C" fn map_unique(p: Ptr) -> Ptr {
    if (*p).rc == 1 {
        return p;
    }
    let old = &*(p as *const Map);
    if old.heap != 0 {
        for v in old.map.values() {
            retain(*v as usize as Ptr);
        }
    }
    let copy = Map {
        head: Header {
            rc: 1,
            kind: KIND_MAP,
        },
        heap: old.heap,
        map: old.map.clone(),
    };
    release(p);
    born();
    Box::into_raw(Box::new(copy)) as Ptr
}

/// Inserts or replaces an entry. A heap value's reference moves into the map.
#[no_mangle]
pub unsafe extern "C" fn map_set(p: Ptr, key: Ptr, bits: u64) -> Ptr {
    let p = map_unique(p);
    let map = &mut *(p as *mut Map);
    let old = map.map.insert(str_of(key).to_string(), bits);
    if let (Some(old), true) = (old, map.heap != 0) {
        release(old as usize as Ptr);
    }
    p
}

#[no_mangle]
pub unsafe extern "C" fn map_get(p: Ptr, key: Ptr) -> u64 {
    let map = &*(p as *const Map);
    match map.map.get(str_of(key)) {
        Some(v) => *v,
        None => fail(format!("key not found: {:?}", str_of(key))),
    }
}

/// Moves a heap value out of a map this caller already owns uniquely.
#[no_mangle]
pub unsafe extern "C" fn map_take(p: Ptr, key: Ptr) -> u64 {
    let map = &mut *(p as *mut Map);
    match map.map.get_mut(str_of(key)) {
        Some(v) => std::mem::replace(v, 0),
        None => fail(format!("key not found: {:?}", str_of(key))),
    }
}

/// `Option<V>` object: `Some(value)` when `found`, else `None`. `heap`: the
/// payload is an owned pointer; `retain` it for the new object (a value that
/// stays in its container) or move the caller's reference in.
unsafe fn option_obj(
    found: Option<u64>,
    some_tag: u32,
    none_tag: u32,
    nslots: u32,
    heap: bool,
    retain_payload: bool,
) -> Ptr {
    match found {
        Some(bits) => {
            let o = obj_new(nslots, if heap { 1 << 1 } else { 0 });
            *slot_ptr(o, 0) = some_tag as u64;
            *slot_ptr(o, 1) = bits;
            if heap && retain_payload {
                retain(bits as usize as Ptr);
            }
            o
        }
        None => {
            let o = obj_new(nslots, 0);
            *slot_ptr(o, 0) = none_tag as u64;
            o
        }
    }
}

/// `map.get(key)` as an `Option` object.
#[no_mangle]
pub unsafe extern "C" fn map_get_opt(
    p: Ptr,
    key: Ptr,
    some_tag: u32,
    none_tag: u32,
    nslots: u32,
    _flags: u32,
) -> Ptr {
    let map = &*(p as *const Map);
    option_obj(
        map.map.get(str_of(key)).copied(),
        some_tag,
        none_tag,
        nslots,
        map.heap != 0,
        true,
    )
}

#[no_mangle]
pub unsafe extern "C" fn map_len(p: Ptr) -> u32 {
    (*(p as *const Map)).map.len() as u32
}

#[no_mangle]
pub unsafe extern "C" fn map_has(p: Ptr, key: Ptr) -> u32 {
    (*(p as *const Map)).map.contains_key(str_of(key)) as u32
}

// ---- lists ----------------------------------------------------------------------------

fn buf_layout(cap: u32, esize: u32) -> Layout {
    Layout::from_size_align((cap as usize * esize as usize).max(1), 8).unwrap()
}

#[no_mangle]
pub extern "C" fn list_new(cap: u32, esize: u32, heap: u32) -> Ptr {
    born();
    let ptr = if cap > 0 {
        unsafe { alloc_zeroed(buf_layout(cap, esize)) }
    } else {
        std::ptr::null_mut()
    };
    Box::into_raw(Box::new(List {
        head: Header {
            rc: 1,
            kind: KIND_LIST,
        },
        len: 0,
        cap,
        ptr,
        esize,
        heap,
    })) as Ptr
}

/// A list this caller may write to: `p` itself when unshared, otherwise a
/// private copy (and the reference to `p` is released).
#[no_mangle]
pub unsafe extern "C" fn list_unique(p: Ptr) -> Ptr {
    if (*p).rc == 1 {
        return p;
    }
    let old = &*(p as *const List);
    let copy = list_new(old.len.max(1), old.esize, old.heap) as *mut List;
    std::ptr::copy_nonoverlapping(old.ptr, (*copy).ptr, (old.len * old.esize) as usize);
    (*copy).len = old.len;
    if old.heap != 0 {
        for i in 0..old.len as usize {
            retain(*(old.ptr.add(i * 8) as *mut u64) as usize as Ptr);
        }
    }
    release(p);
    copy as Ptr
}

unsafe fn write_elem(l: &mut List, index: u32, bits: u64) {
    let src = bits.to_le_bytes();
    std::ptr::copy_nonoverlapping(
        src.as_ptr(),
        l.ptr.add(index as usize * l.esize as usize),
        l.esize as usize,
    );
}

/// Slow path of `push`: the list is shared or full. A heap element's
/// reference moves into the list.
#[no_mangle]
pub unsafe extern "C" fn list_push(p: Ptr, bits: u64) -> Ptr {
    let p = list_unique(p);
    let l = &mut *(p as *mut List);
    if l.len == l.cap {
        let new_cap = (l.cap * 2).max(4);
        l.ptr = if l.cap == 0 {
            alloc_zeroed(buf_layout(new_cap, l.esize))
        } else {
            realloc(
                l.ptr,
                buf_layout(l.cap, l.esize),
                (new_cap * l.esize) as usize,
            )
        };
        l.cap = new_cap;
    }
    write_elem(l, l.len, bits);
    l.len += 1;
    p
}

fn oob(index: i64, len: u32) -> ! {
    fail(format!("index {index} out of bounds (len={len})"))
}

/// Slow path of `xs[i] = v`: the list is shared, the index is bad, or the
/// elements are heap values (whose old value is released here).
#[no_mangle]
pub unsafe extern "C" fn list_set(p: Ptr, index: i64, bits: u64) -> Ptr {
    let len = (*(p as *mut List)).len;
    if index < 0 || index as u64 >= len as u64 {
        oob(index, len);
    }
    let p = list_unique(p);
    let l = &mut *(p as *mut List);
    if l.heap != 0 {
        release(*(l.ptr.add(index as usize * 8) as *mut u64) as usize as Ptr);
    }
    write_elem(l, index as u32, bits);
    p
}

/// Moves a heap element out of a list this caller already owns uniquely.
#[no_mangle]
pub unsafe extern "C" fn list_take(p: Ptr, index: i64) -> u64 {
    let l = &mut *(p as *mut List);
    if index < 0 || index as u64 >= l.len as u64 {
        oob(index, l.len);
    }
    let slot = l.ptr.add(index as usize * 8) as *mut u64;
    std::mem::replace(&mut *slot, 0)
}

/// `list.pop()` on a list this caller already owns uniquely: the removed
/// element's reference moves into the returned `Option` object. `flags` bit 0:
/// payload is a heap value; bit 1: payload is a signed 32-bit integer.
#[no_mangle]
pub unsafe extern "C" fn list_pop_opt(
    p: Ptr,
    some_tag: u32,
    none_tag: u32,
    nslots: u32,
    flags: u32,
) -> Ptr {
    let l = &mut *(p as *mut List);
    if l.len == 0 {
        return option_obj(None, some_tag, none_tag, nslots, false, false);
    }
    l.len -= 1;
    let at = l.ptr.add((l.len * l.esize) as usize);
    let bits = if l.heap != 0 {
        *(at as *const u64)
    } else {
        let mut raw = [0u8; 8];
        std::ptr::copy_nonoverlapping(at, raw.as_mut_ptr(), l.esize as usize);
        u64::from_le_bytes(raw)
    };
    let bits = if l.heap == 0 && l.esize == 4 && flags & 2 != 0 {
        bits as u32 as i32 as i64 as u64
    } else {
        bits
    };
    option_obj(
        Some(bits),
        some_tag,
        none_tag,
        nslots,
        flags & 1 != 0,
        false,
    )
}

#[no_mangle]
pub unsafe extern "C" fn list_oob(p: Ptr, index: i64) {
    oob(index, (*(p as *mut List)).len);
}

// ---- bridge to the reference implementation ---------------------------------------------------

/// Runs a runtime call on the interpreter. `args` holds the operands as
/// 64-bit bits (borrowed) and `tys` their type ids; for a call that mutates its
/// first operand, `args[0]` receives the new object (one owned reference) that
/// replaces it.
#[no_mangle]
pub unsafe extern "C" fn call(f: u32, n: u32, args: *mut u64, tys: *const u32, dst_ty: u32) -> u64 {
    let m = module();
    let n = n as usize;
    let f = RT_FNS[f as usize];
    let arg_tys: Vec<TyId> = (0..n).map(|i| TyId(*tys.add(i))).collect();
    let vals: Vec<Val> = (0..n)
        .map(|i| to_val(m, arg_tys[i], *args.add(i)))
        .collect();
    let dst_ty = TyId(dst_ty);
    match state().interp.rt_values(f, &arg_tys, dst_ty, vals) {
        Ok((result, after)) => {
            if f.mutates_first() {
                *args = from_val(m, arg_tys[0], &after[0]);
            }
            from_val(m, dst_ty, &result)
        }
        Err(trap) => fail(trap.msg),
    }
}

/// `a <op> b` on two values of type `ty`.
#[no_mangle]
pub extern "C" fn cmp(ty: u32, op: u32, a: u64, b: u64) -> u32 {
    let m = module();
    let ty = TyId(ty);
    let (va, vb) = (to_val(m, ty, a), to_val(m, ty, b));
    match state()
        .interp
        .compare_values(ty, CMP_OPS[op as usize], &va, &vb)
    {
        Ok(r) => r as u32,
        Err(trap) => fail(trap.msg),
    }
}

/// The text `"{value}"` produces.
#[no_mangle]
pub extern "C" fn to_str(ty: u32, bits: u64) -> Ptr {
    let m = module();
    let ty = TyId(ty);
    new_str(display(m, ty, &to_val(m, ty, bits)))
}

#[no_mangle]
pub extern "C" fn print(ty: u32, bits: u64, newline: u32) {
    let m = module();
    let ty = TyId(ty);
    let text = display(m, ty, &to_val(m, ty, bits));
    unsafe { host_print(text.as_ptr(), text.len() as u32, newline) };
}

/// A string with the value rendered like the interpreter renders a result.
/// The value stays owned by the caller.
#[no_mangle]
pub extern "C" fn render_value(ty: u32, bits: u64) -> Ptr {
    let m = module();
    let ty = TyId(ty);
    new_str(render(m, ty, &to_val(m, ty, bits)))
}
