// rune-evaluator/env.rs

use crate::value::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Env {
    stack: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Self { stack: vec![HashMap::new()] }
    }

    pub fn push(&mut self) {
        self.stack.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn define(&mut self, name: &str, val: Value) {
        self.stack.last_mut().unwrap().insert(name.to_string(), val);
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        for frame in self.stack.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}
