// runtime/scope.rs

use crate::value::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct RuntimeScopeStack {
    frames: Vec<HashMap<String, Value>>,
}

impl RuntimeScopeStack {
    pub fn new() -> Self {
        Self { frames: vec![HashMap::new()] }
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    pub fn define(&mut self, name: &str, val: Value) {
        self.frames.last_mut().unwrap().insert(name.into(), val);
    }

    pub fn assign(&mut self, name: &str, val: Value) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.into(), val);
                return true;
            }
        }
        false
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        for frame in self.frames.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}
