use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Generic(String, Vec<Type>),
    Fn(Box<Type>, Vec<Type>),

    Array(Box<Type>),
    Tuple(Vec<Type>),
    Map(Box<Type>),

    /// Inference variable, only ever seen while a program is being checked.
    /// `SemanticModel` never contains one.
    Var(u32),

    Unknown,
}

impl Type {
    /// True when this type is, or contains, `Unknown` or an unsolved
    /// inference variable.
    pub fn has_unknown(&self) -> bool {
        match self {
            Type::Unknown | Type::Var(_) => true,
            Type::Array(t) | Type::Map(t) | Type::Tensor(t) => t.has_unknown(),
            Type::Tuple(items) => items.iter().any(Type::has_unknown),
            Type::Generic(_, args) => args.iter().any(Type::has_unknown),
            Type::Fn(ret, params) => ret.has_unknown() || params.iter().any(Type::has_unknown),
            _ => false,
        }
    }

    /// `i32`, `f64`, ... (the sized numeric types); `Number` is the
    /// unsuffixed literal type and is not included.
    pub fn is_sized_numeric(&self) -> bool {
        matches!(self, Type::Simple(n) if is_numeric_name(n))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Number) || self.is_sized_numeric()
    }
}

pub(crate) fn is_numeric_name(name: &str) -> bool {
    matches!(name, "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64")
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
