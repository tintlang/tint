mod formatting;
mod methods;

use crate::env::Env;
use std::{collections::HashMap, rc::Rc};

/// Runtime value produced by the Tint evaluator.
#[derive(Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Unit,

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

    Namespace {
        name: String,
        items: Rc<Env>,
    },
}
