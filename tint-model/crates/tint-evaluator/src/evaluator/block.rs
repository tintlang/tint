use crate::{EvalHost};
use crate::eval_host::Flow;
use tint_ast::Block;

pub fn eval_block_flow<H: EvalHost>(host: &mut H, block: &Block) -> Flow {
    let mut last = Flow::Value(crate::value::Value::Unit);

    for stmt in &block.stmts {
        let flow = crate::eval_stmt::eval_stmt(host, stmt);

        match &flow {
            Flow::Value(_) => {
                last = flow;
            }
            Flow::Break | Flow::Continue | Flow::Return(_) => {
                return flow;
            }
        }
    }

    last
}
