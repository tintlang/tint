// runtime/state/cell.rs

use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub struct StateCell<T> {
    value: RefCell<T>,
    version: Cell<u64>,
}

impl<T> StateCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
            version: Cell::new(0),
        }
    }

    pub fn get(&self) -> T where T: Clone {
        self.value.borrow().clone()
    }

    pub fn set(&self, v: T) {
        *self.value.borrow_mut() = v;
        self.version.set(self.version.get() + 1);
    }

    pub fn version(&self) -> u64 {
        self.version.get()
    }
}

