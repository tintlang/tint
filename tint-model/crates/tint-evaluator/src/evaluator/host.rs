use std::rc::Rc;

use tint_ast::{Block, Expr, Pattern, Span};

use crate::{env::Env, errors::EvalResult, value::Value};

/// Control-flow result propagated while evaluating a block.
#[derive(Debug, Clone)]
pub enum Flow {
    Value(Value),
    Return(Value),
    Break,
    Continue,
}

pub trait EvalHost {
    fn load_var(&mut self, name: &str, span: Span) -> Value;
    fn define_var(&mut self, name: &str, value: Value);

    fn push_scope(&mut self);
    fn pop_scope(&mut self);
    fn eval_expr(&mut self, expr: &Expr) -> Value;
    fn eval_block(&mut self, block: &Block) -> Flow;

    fn assign_to(&mut self, lhs: &Expr, value: Value) -> bool;

    /// Resolve and invoke a named function.
    fn call_fn(
        &mut self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> EvalResult<Value>;

    /// Invoke a user-defined function by name.
    fn call_user_fn(
        &mut self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> EvalResult<Value>;

    fn set_var(&mut self, name: &str, value: Value);

    fn call_value(&mut self, value: Value, args: &[Value], span: Span) -> Value;
    fn namespace_lookup(&mut self, ns: Value, item: &str) -> Value;
    fn field_lookup(&mut self, obj: Value, field: &str) -> Value;
    fn index_lookup(&mut self, obj: Value, index: Value) -> Value;
    fn capture_env(&mut self) -> Rc<Env>;

    fn eval_block_flow(&mut self, block: &Block) -> Flow;
    fn match_pattern(&mut self, value: &Value, pat: &Pattern) -> bool;

    fn bind_pattern(&mut self, pat: &Pattern, value: &Value) -> bool;
    fn apply_compound(&mut self, left: &Value, op: &str, right: &Value) -> Value;
}
