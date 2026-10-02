//! Allocator of the wasm runtime. Programs allocate many small objects (strings,
//! list/map headers, struct and enum nodes); the general-purpose allocator spends
//! more on finding a chunk than the object is worth. Sizes up to 256 bytes come
//! from per-size free lists carved out of big blocks and are never given back to
//! the system; anything larger (or aligned past 16) goes to the system allocator.
//! wasm has one thread here, so no locking.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;

const MAX_SMALL: usize = 256;
const CLASSES: usize = MAX_SMALL / 16;
const BLOCK: usize = 64 * 1024;

struct State {
    free: [*mut u8; CLASSES],
    bump: *mut u8,
    end: *mut u8,
}

pub struct SmallAlloc(UnsafeCell<State>);

unsafe impl Sync for SmallAlloc {}

impl SmallAlloc {
    pub const fn new() -> Self {
        SmallAlloc(UnsafeCell::new(State {
            free: [std::ptr::null_mut(); CLASSES],
            bump: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
        }))
    }
}

#[inline]
fn class(size: usize) -> usize {
    (size.max(1) + 15) / 16 - 1
}

#[inline]
fn small(layout: &Layout) -> bool {
    layout.size() <= MAX_SMALL && layout.align() <= 16
}

unsafe impl GlobalAlloc for SmallAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !small(&layout) {
            return System.alloc(layout);
        }
        let st = &mut *self.0.get();
        let c = class(layout.size());
        let head = st.free[c];
        if !head.is_null() {
            st.free[c] = *(head as *mut *mut u8);
            return head;
        }
        let bytes = (c + 1) * 16;
        if (st.end as usize) - (st.bump as usize) < bytes {
            let block = System.alloc(Layout::from_size_align_unchecked(BLOCK, 16));
            if block.is_null() {
                return block;
            }
            st.bump = block;
            st.end = block.add(BLOCK);
        }
        let p = st.bump;
        st.bump = p.add(bytes);
        p
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !small(&layout) {
            return System.dealloc(ptr, layout);
        }
        let st = &mut *self.0.get();
        let c = class(layout.size());
        *(ptr as *mut *mut u8) = st.free[c];
        st.free[c] = ptr;
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        match (small(&layout), small(&new_layout)) {
            (false, false) => System.realloc(ptr, layout, new_size),
            (true, true) if class(layout.size()) == class(new_size) => ptr,
            _ => {
                let fresh = self.alloc(new_layout);
                if !fresh.is_null() {
                    std::ptr::copy_nonoverlapping(ptr, fresh, layout.size().min(new_size));
                    self.dealloc(ptr, layout);
                }
                fresh
            }
        }
    }
}
