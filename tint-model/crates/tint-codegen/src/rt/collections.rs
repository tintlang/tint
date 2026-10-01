// ---- strings ---------------------------------------------------------------

fn new_str(s: String) -> Ptr {
    Box::into_raw(Box::new(Str {
        head: Header {
            rc: 1,
            kind: KIND_STR,
        },
        s,
    })) as Ptr
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
        head: Header {
            rc: 1,
            kind: KIND_MAP,
        },
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
            head: Header {
                rc: 1,
                kind: KIND_MAP,
            },
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
        head: Header {
            rc: 1,
            kind: KIND_LIST,
        },
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
    Box::into_raw(Box::new(new_list(
        cap.max(0) as usize,
        esize as usize,
        heap != 0,
    ))) as Ptr
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
