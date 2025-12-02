// rune-evaluator/eval_host.rs

use crate::value::Value;
use crate::errors::{EvalError, EvalResult};
use rune_ast::{Expr, Block, Span, Pattern};
use crate::env::Env;
use std::rc::Rc;

/// Control flow inside eval:
/// return / break / continue need to bubble up
#[derive(Debug, Clone)]
pub enum Flow {
    Value(Value),
    Return(Value),
    Break,
    Continue,
}

pub trait EvalHost {
    // Variable System
    fn load_var(&mut self, name: &str, span: Span) -> Value;
    fn define_var(&mut self, name: &str, value: Value);

    fn push_scope(&mut self);
    fn pop_scope(&mut self);

    // Expression / Block Evaluators
    // (Evaluator calls these)
    fn eval_expr(&mut self, expr: &Expr) -> Value;
    fn eval_block(&mut self, block: &Block) -> Flow;

    // Function Calls

    /// Entry point for ALL function calls.
    /// Evaluator calls this from `Expr::Call`.
    fn call_fn(
        &mut self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> EvalResult<Value>;

    /// Called only for user-defined functions.
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
}
