// rune-evaluator/env.rs

use std::{collections::HashMap, rc::Rc};
use crate::value::Value;

/// One environment frame (scope)
#[derive(Debug, Default, Clone)]
pub struct Env {
    /// local variable frames (stack)
    frames: Vec<HashMap<String, Value>>,

    /// structs by name
    structs: HashMap<String, StructInfo>,

    /// enums by name
    enums: HashMap<String, EnumInfo>,

    /// nested namespaces (modules)
    namespaces: HashMap<String, Rc<Env>>,
}

/// Struct definition
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<String>,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<String>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            frames: vec![HashMap::new()],
            structs: HashMap::new(),
            enums: HashMap::new(),
            namespaces: HashMap::new(),
        }
    }

    // SCOPE MANAGEMENT
    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn set(&mut self, name: &str, val: Value) {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.to_string(), val);
                return;
            }
        }
        panic!("Assign to undefined variable '{}'", name);
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    // VARIABLES
    pub fn define(&mut self, name: &str, val: Value) {
        self.frames.last_mut().unwrap()
            .insert(name.to_string(), val);
    }

    pub fn assign(&mut self, name: &str, val: Value) -> bool {
        // assign only to first matching frame
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.to_string(), val);
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

    // STRUCT DEFINITIONS
    pub fn define_struct(&mut self, name: &str, fields: Vec<String>) {
        self.structs.insert(name.to_string(), StructInfo { name: name.into(), fields });
    }

    pub fn get_struct(&self, name: &str) -> Option<StructInfo> {
        self.structs.get(name).cloned()
    }

    // ENUM DEFINITIONS
    pub fn define_enum(&mut self, name: &str, variants: Vec<String>) {
        self.enums.insert(name.to_string(), EnumInfo { name: name.into(), variants });
    }

    pub fn get_enum(&self, name: &str) -> Option<EnumInfo> {
        self.enums.get(name).cloned()
    }

    // NAMESPACES (MODULES)
    pub fn define_namespace(&mut self, name: &str, env: Rc<Env>) {
        self.namespaces.insert(name.into(), env);
    }

    pub fn lookup_namespace(&self, name: &str) -> Option<Rc<Env>> {
        self.namespaces.get(name).cloned()
    }

    pub fn extend_from(&mut self, other: &Env) {
        for frame in &other.frames {
            for (k, v) in frame {
                self.define(k, v.clone());
            }
        }
    }

}
