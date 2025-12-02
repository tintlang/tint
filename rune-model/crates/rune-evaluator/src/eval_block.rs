// rune-evaluator/eval_block.rs

use crate::{value::Value, EvalHost};
use rune_ast::Block;

pub fn eval_block<H: EvalHost>(host: &mut H, block: &Block) -> Value {
    let mut last = Value::Unit;

    for stmt in &block.stmts {
        last = crate::eval_stmt::eval_stmt(host, stmt);
    }

    last
}
