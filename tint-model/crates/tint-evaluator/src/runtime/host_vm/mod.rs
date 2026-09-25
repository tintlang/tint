mod access;
mod calls;
mod eval_host;
mod patterns;

use std::collections::HashMap;

use tint_ast::FnDecl;

use crate::{env::Env, value::Value};

/// Default runtime host used by the evaluator.
pub struct HostVM {
    pub env: Env,
    pub functions: HashMap<String, FnDecl>,
    pub modules: HashMap<String, Value>,
}

impl HostVM {
    pub fn new() -> Self {
        Self {
            env: Env::new(),
            functions: HashMap::new(),
            modules: HashMap::new(),
        }
    }
}

impl Default for HostVM {
    fn default() -> Self {
        Self::new()
    }
}
