use crate::eval_block::eval_block_flow;
use crate::eval_expr::eval_expr;
use crate::eval_host::Flow;
use crate::eval_pattern::bind_pattern;
use crate::{value::Value, EvalHost};
use tint_ast::{Block, Expr, FnBody, FnDecl, Span};

#[derive(Debug, Clone)]
pub enum FnBodyKind {
    Block(Block),
    Expr(Expr),
}

pub fn eval_user_fn<H: EvalHost>(host: &mut H, f: &FnDecl, args: &[Value], _span: Span) -> Value {
    // Function calls execute in a fresh scope.
    host.push_scope();

    for (param, arg) in f.params.iter().zip(args.iter()) {
        bind_pattern(host, &param.pattern, arg);
    }
    let result = match &f.body {
        FnBody::Expr(expr) => Value::from(eval_expr(host, expr)),

        FnBody::Block(block) => {
            match eval_block_flow(host, block) {
                Flow::Value(v) => v,
                Flow::Return(v) => v,
                Flow::Break | Flow::Continue => {
                    // break/continue inside fn behave like early return Unit
                    Value::Unit
                }
            }
        }
    };

    // Discard bindings created for this call.
    host.pop_scope();

    result
}
