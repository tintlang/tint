// runtime/state/store.rs

use std::any::Any;
use std::collections::{HashMap, HashSet};

use super::WatchCallback;
use super::{ComputedCell, SignalCell, StateCell, WatchEntry};

pub type StateId = usize;

pub struct StateStore {
    next_id: StateId,

    states: HashMap<StateId, Box<dyn AnyState>>,
    signals: HashMap<StateId, Box<dyn AnySignal>>,
    computed: HashMap<StateId, Box<dyn AnyComputed>>,
    watchers: Vec<WatchEntry>,

    dirty: HashSet<StateId>,
}

pub trait AnyState: Any {
    fn as_any(&self) -> &dyn Any;
}

pub trait AnySignal: Any {
    fn as_any(&self) -> &dyn Any;
}

pub trait AnyComputed: Any {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Clone + 'static> AnyState for StateCell<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T: Clone + 'static> AnySignal for SignalCell<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T: Clone + 'static> AnyComputed for ComputedCell<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            states: HashMap::new(),
            signals: HashMap::new(),
            computed: HashMap::new(),
            watchers: Vec::new(),
            dirty: HashSet::new(),
        }
    }

    pub fn create_state<T: Clone + 'static>(&mut self, value: T) -> StateId {
        let id = self.next_id;
        self.next_id += 1;

        self.states.insert(id, Box::new(StateCell::new(value)));
        id
    }

    pub fn get_state<T: Clone + 'static>(&self, id: StateId) -> T {
        let any = self.states.get(&id).unwrap().as_any();
        let cell = any.downcast_ref::<StateCell<T>>().unwrap();
        cell.get()
    }

    pub fn set_state<T: Clone + 'static>(&mut self, id: StateId, val: T) {
        let any = self.states.get(&id).unwrap().as_any();
        let cell = any.downcast_ref::<StateCell<T>>().unwrap();
        cell.set(val);
        self.mark_dirty(id);
    }

    fn mark_dirty(&mut self, id: StateId) {
        self.dirty.insert(id);
    }

    // COMPUTED
    pub fn create_computed<T, F>(&mut self, deps: Vec<StateId>, compute: F) -> StateId
    where
        T: Clone + 'static,
        F: Fn(&StateStore) -> T + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        let cell = ComputedCell::new(compute, deps);
        cell.force_update(self);

        self.computed.insert(id, Box::new(cell));
        id
    }

    pub fn get_computed<T: Clone + 'static>(&self, id: StateId) -> T {
        let any = self.computed.get(&id).unwrap().as_any();
        let cell = any.downcast_ref::<ComputedCell<T>>().unwrap();
        cell.get()
    }

    pub fn watch(&mut self, target: StateId, callback: WatchCallback) {
        self.watchers.push(WatchEntry { target, callback });
    }

    pub fn flush(&mut self) {
        if self.dirty.is_empty() {
            return;
        }

        let dirty = self.dirty.clone();
        self.dirty.clear();

        // recompute all computed
        for (_id, any) in &self.computed {
            if let Some(c) = any.as_any().downcast_ref::<ComputedCell<i32>>() {
                c.force_update(self);
            }
        }

        // watchers
        for w in &self.watchers {
            if dirty.contains(&w.target) {
                (w.callback)();
            }
        }
    }
}
