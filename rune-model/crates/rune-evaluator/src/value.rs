// rune-evaluator/value.rs

use std::{fmt, rc::Rc};
use crate::env::Env;
use rune_ast::Type;

/// Runtime value for RuneLang
#[derive(Clone)]
pub enum Value {
    // Primitive values
    Number(f64),
    String(String),
    Bool(bool),
    Unit,

    Tuple(Vec<Value>),

    // Struct instance:  Point { x:1, y:2 }
    StructInstance {
        name: String,
        fields: Vec<(String, Value)>,
    },

    // Enum instance: Ok(val), Error(msg)
    EnumInstance {
        enum_name: String,
        variant: String,
        args: Vec<Value>,
    },

    // List / array: [a, b, c]
    List(Vec<Value>),

    Map(std::collections::HashMap<String, Value>),

    // Runtime lambda (|x| x+1)
    Lambda {
        params: Vec<String>,
        body: Box<rune_ast::Expr>,
        closure: Rc<Env>,
    },

    // User-defined function (FnDecl)
    Function {
        name: String,
        params: Vec<String>,
        body: crate::eval_fn::FnBodyKind,
        env: Rc<Env>,
        async_: bool,
    },

    // Host function (Rust)
    HostFunction(fn(Vec<Value>) -> Value),

    // Namespace (module), used for module::item
    Namespace {
        name: String,
        items: Rc<Env>,
    },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::String(s) => write!(f, "{}", s),
            Value::Unit => write!(f, "()"),

            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }

            // ✔ ARRAY / LIST
            Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }

            // ✔ STRUCT
            Value::StructInstance { name, fields } => {
                write!(f, "{} {{ ", name)?;
                let mut first = true;
                for (k, v) in fields {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }

            // ✔ ENUM
            Value::EnumInstance { enum_name, variant, args } => {
                write!(f, "{}::{}(", enum_name, variant)?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }

            Value::Map(map) => {
                write!(f, "map {{ ")?;
                let mut first = true;
                for (k, v) in map {
                    if !first { write!(f, ", ")?; }
                    first = false;
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }

            Value::Lambda { .. } => write!(f, "<lambda>"),
            Value::Function { name, .. } => write!(f, "<fn {}>", name),
            Value::HostFunction(_) => write!(f, "<host-fn>"),
            Value::Namespace { name, .. } => write!(f, "<namespace {}>", name),
        }
    }
}


impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Unit => write!(f, "unit"),

            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", item)?;
                }
                write!(f, ")")
            }

            Value::StructInstance { name, fields } => {
                write!(f, "{} {{ ", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    write!(f, "{}: {:?}", k, v)?;
                    if i + 1 < fields.len() { write!(f, ", ")?; }
                }
                write!(f, " }}")
            }

            Value::EnumInstance { enum_name, variant, args } => {
                write!(f, "{}::{}(", enum_name, variant)?;
                for (i, v) in args.iter().enumerate() {
                    write!(f, "{:?}", v)?;
                    if i + 1 < args.len() { write!(f, ", ")?; }
                }
                write!(f, ")")
            }

            Value::List(list) => write!(f, "{:?}", list),

            Value::Map(map) => {
                write!(f, "map {{ ")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    write!(f, "{}: {:?}", k, v)?;
                    if i + 1 < map.len() { write!(f, ", ")?; }
                }
                write!(f, " }}")
            }

            Value::Lambda { .. } =>
                write!(f, "<lambda>"),

            Value::Function { name, .. } =>
                write!(f, "<fn {}>", name),

            Value::HostFunction(_) =>
                write!(f, "<host-fn>"),

            Value::Namespace { name, .. } =>
                write!(f, "<namespace {}>", name),
        }
    }
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self { Value::Number(n) => Some(*n), _ => None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self { Value::Bool(b) => Some(*b), _ => None }
    }

    /// Set struct field: obj.field = value
    pub fn set_field(&mut self, field: &str, new_value: Value) {
        match self {
            Value::StructInstance { fields, .. } => {
                for (name, val) in fields.iter_mut() {
                    if name == field {
                        *val = new_value;
                        return;
                    }
                }
                panic!("Field '{}' not found in struct", field);
            }

            Value::Map(map) => {
                map.insert(field.to_string(), new_value);
            }

            _ => panic!("Cannot assign field '{}' on non-struct value {:?}", field, self),
        }
    }

    /// Set array/list index: arr[i] = value
    pub fn set_index(&mut self, index: i64, new_value: Value) {
        match self {
            Value::List(items) => {
                let idx = index as usize;
                if idx >= items.len() {
                    panic!("Index {} out of bounds", index);
                }
                items[idx] = new_value;
            }

            Value::Tuple(items) => {
                let idx = index as usize;
                if idx >= items.len() {
                    panic!("Tuple index {} out of bounds", index);
                }
                items[idx] = new_value;
            }

            _ => panic!("Cannot index-assign on non-list value {:?}", self),
        }
    }

    pub fn force_bool(&self) -> bool {
        self.as_bool().unwrap_or(false)
    }
        pub fn as_int(&self) -> i64 {
        match self {
            Value::Number(n) => *n as i64,
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn matches_type(&self, ty: &Type) -> bool {
        match (self, ty) {
            // -------- PRIMITIVES --------
            (Value::Number(_), Type::Simple(t)) 
                if t == "i32" || t == "f32" => true,

            (Value::Bool(_), Type::Simple(t)) if t == "bool" => true,

            (Value::String(_), Type::Simple(t)) if t == "string" => true,

            (Value::Unit, Type::Unit) => true,

            // -------- STRUCT --------
            (Value::StructInstance { name, .. }, Type::Simple(t)) 
                if name == t => true,

            // -------- ENUM --------
            (Value::EnumInstance { enum_name, .. }, Type::Simple(t)) 
                if enum_name == t => true,

            // -------- TUPLES (когда Type::Tuple будет добавлен) --------
            // (Value::Tuple(vals), Type::Tuple(types))
            //     if vals.len() == types.len() => true,

            // -------- GENERIC (Option<T>, Result<T>) пока заглушка --------
            (v, Type::Generic(tname, _args)) => {
                // временно считаем, что generic всегда проходит
                // позже добавим строгую проверку
                true
            }

            _ => false,
        }
    }
}

