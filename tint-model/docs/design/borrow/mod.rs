// runtime/borrow/mod.rs
//
// IDEA -- not wired into TintVM. An early sketch toward a Rust-style
// borrow-checking concept for Tint values; `BorrowManager` here only
// tracks a single global lock, nothing calls `lock`/`unlock`, and no
// value in the evaluator consults it. Direct Rust interop
// (`TintVM::register_native`) turned out to be the actual answer to
// "how does Tint interact with Rust's ownership rules" -- it hands the
// whole question to real Rust code instead of reimplementing a borrow
// checker. Kept as reference in case real per-value borrow tracking is
// picked up again later, not as working code.
#![allow(dead_code)]

pub mod intrinsic;
pub mod lock;

pub struct BorrowManager {
    locked: Option<usize>,
}

impl BorrowManager {
    pub fn new() -> Self {
        Self { locked: None }
    }

    pub fn lock(&mut self, id: usize) -> bool {
        if self.locked.is_some() {
            return false;
        }
        self.locked = Some(id);
        true
    }

    pub fn unlock(&mut self) {
        self.locked = None;
    }
}
