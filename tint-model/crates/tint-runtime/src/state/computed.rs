use crate::state::StateStore;
use std::cell::RefCell;
use std::rc::Rc;

use super::store::StateId;
pub struct ComputedCell<T: Clone> {
    pub compute: Rc<dyn Fn(&StateStore) -> T>,
    pub value: RefCell<Option<T>>,
    pub deps: Vec<StateId>,
}

impl<T: Clone> ComputedCell<T> {
    pub fn new<F>(compute: F, deps: Vec<StateId>) -> Self
    where
        F: 'static + Fn(&StateStore) -> T,
    {
        Self {
            compute: Rc::new(compute),
            value: RefCell::new(None),
            deps,
        }
    }

    pub fn force_update(&self, store: &StateStore) {
        let v = (self.compute)(store);
        *self.value.borrow_mut() = Some(v);
    }

    pub fn get(&self) -> T {
        self.value.borrow().clone().unwrap()
    }
}
