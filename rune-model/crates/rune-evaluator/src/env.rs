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

     pub fn lookup_path(&self, path: &[String]) -> Option<Value> {
        let mut current: Option<Rc<Env>> = Some(Rc::new(self.clone()));

        for (i, seg) in path.iter().enumerate() {
            let env = current.clone()?;

            // Если это последний сегмент → ищем значение или namespace
            if i == path.len() - 1 {
                if let Some(v) = env.lookup(seg) {
                    return Some(v);
                }
                if let Some(ns) = env.lookup_namespace(seg) {
                    return Some(Value::Namespace {
                        name: seg.clone(),
                        items: ns,
                    });
                }
                return None;
            }

            // иначе сегмент → должен быть namespace
            current = env.lookup_namespace(seg);
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

    pub fn define_alias(&mut self, alias: &str, value: Value) {
        self.frames.last_mut().unwrap().insert(alias.to_string(), value);
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
        let dst = self.frames.last_mut().unwrap();
        let src = &other.frames[0];

        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }

}
