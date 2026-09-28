use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Simple(String),
    Unit,
    Number,
    String,
    Bool,

    // UI types
    UI,
    UIChild,
    UIChildren,
    Slot,

    // Resources
    Buffer,
    Image,
    Tensor(Box<Type>),

    // User defined
    Struct(String),
    Enum(String),
    Fn(Box<Type>, Vec<Type>),

    Array(Box<Type>),
    Tuple(Vec<Type>),
    Map(Box<Type>),

    Unknown,
}

#[derive(Default)]
pub struct TypeTable {
    pub vars: HashMap<String, Type>,
}

impl TypeTable {
    pub fn set(&mut self, name: &str, ty: Type) {
        self.vars.insert(name.to_string(), ty);
    }

    pub fn get(&self, name: &str) -> Option<Type> {
        self.vars.get(name).cloned()
    }
}
