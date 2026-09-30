// runtime/scope.rs

use crate::value::RuntimeValue;
use std::collections::HashMap as StdHashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// FxHash-style hasher: scope keys are short identifiers, and SipHash (plus a
/// random seed per frame) was a visible share of every UI render.
#[derive(Default)]
pub struct FxHasher(u64);

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut v = 0u64;
            for (i, b) in chunk.iter().enumerate() {
                v |= (*b as u64) << (i * 8);
            }
            self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.0 = (self.0.rotate_left(5) ^ i as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

pub type HashMap<K, V> = StdHashMap<K, V, BuildHasherDefault<FxHasher>>;

#[derive(Debug, Default)]
pub struct RuntimeScopeStack {
    frames: Vec<HashMap<String, RuntimeValue>>,
}

impl RuntimeScopeStack {
    pub fn new() -> Self {
        Self {
            frames: vec![HashMap::default()],
        }
    }

    pub fn set(&mut self, name: &str, value: RuntimeValue) {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.to_string(), value);
                return;
            }
        }

        panic!("Assign to undefined variable '{}'", name);
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::default());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    pub fn define(&mut self, name: &str, val: RuntimeValue) {
        self.frames.last_mut().unwrap().insert(name.into(), val);
    }

    pub fn assign(&mut self, name: &str, val: RuntimeValue) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.into(), val);
                return true;
            }
        }
        false
    }

    /// Number of scope frames currently on the stack.
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Like `lookup`, but also reports the index of the frame that holds
    /// the binding and borrows the value instead of cloning it.
    pub fn lookup_ref(&self, name: &str) -> Option<(usize, &RuntimeValue)> {
        for (index, frame) in self.frames.iter().enumerate().rev() {
            if let Some(v) = frame.get(name) {
                return Some((index, v));
            }
        }
        None
    }

    pub fn lookup(&self, name: &str) -> Option<RuntimeValue> {
        for frame in self.frames.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    pub fn values(&self) -> Vec<(String, RuntimeValue)> {
        let mut values: StdHashMap<String, RuntimeValue> = StdHashMap::new();
        for frame in &self.frames {
            for (name, value) in frame {
                values.insert(name.clone(), value.clone());
            }
        }
        values.into_iter().collect()
    }
}
