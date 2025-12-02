// rune-evaluator/eval_host.rs

use crate::value::Value;
use rune_ast::{Expr, Block, Span};

pub trait EvalHost {
    // -----------------------------
    // Variables
    // -----------------------------
    fn load_var(&mut self, name: &str, span: Span) -> Value;
    fn define_var(&mut self, name: &str, value: Value);

    fn push_scope(&mut self);
    fn pop_scope(&mut self);

    // -----------------------------
    // Eval
    // -----------------------------
    fn eval_expr(&mut self, expr: &Expr) -> Value;
    fn eval_block(&mut self, block: &Block) -> Value;

    // -----------------------------
    // Functions
    // -----------------------------
    fn call_fn(&mut self, name: &str, args: &[Value], span: Span) -> Value;

    /// NEW → evaluator вызывает это для "user-defined" функций
    fn call_user_fn(&mut self, name: &str, args: &[Value], span: Span) -> Value;
}
