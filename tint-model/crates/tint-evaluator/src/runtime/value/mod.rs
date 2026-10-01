mod formatting;
mod methods;

use crate::env::Env;
use std::{collections::HashMap, rc::Rc};

/// A function a host (Rust or JS) can call: Tint lambdas passed to a native
/// function arrive as one, and host closures can be handed to Tint.
#[derive(Clone)]
pub struct Callback(pub Rc<dyn Fn(&[Value]) -> crate::errors::EvalResult<Value>>);

/// Runtime value produced by the Tint evaluator.
#[derive(Clone)]
pub enum Value {
    Number(f64),
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    U8(u8),
    F32(f32),
    F64(f64),
    String(String),
    Bool(bool),
    Unit,
    Propagate(Box<Value>),

    Tuple(Vec<Value>),

    StructInstance {
        name: String,
        fields: Vec<(String, Value)>,
    },

    EnumInstance {
        enum_name: String,
        variant: String,
        args: Vec<Value>,
    },

    List(Vec<Value>),

    Map(HashMap<String, Value>),

    Lambda {
        params: Vec<String>,
        body: Box<tint_ast::Expr>,
        closure: Rc<Env>,
    },

    Function {
        name: String,
        params: Vec<String>,
        body: crate::eval_fn::FnBodyKind,
        env: Rc<Env>,
        async_: bool,
    },

    HostFunction(fn(Vec<Value>) -> Value),

    Callback(Callback),

    Namespace {
        name: String,
        items: Rc<Env>,
    },
}
