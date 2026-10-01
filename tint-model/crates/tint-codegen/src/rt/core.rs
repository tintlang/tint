use crate::{elem_layout, is_heap, ptr_mask};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
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
    let text = CTX.with(|c| {
        c.borrow()
            .msgs
            .get(msg as usize)
            .cloned()
            .unwrap_or_default()
    });
    fail(text);
}

pub extern "C" fn fmod(a: f64, b: f64) -> f64 {
    a % b
}
