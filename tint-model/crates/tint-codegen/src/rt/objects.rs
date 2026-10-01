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
