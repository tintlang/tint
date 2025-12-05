// rune-evaluator/eval_fn.rs

use crate::{value::Value, EvalHost};
use crate::eval_block::eval_block_flow;
use crate::eval_expr::eval_expr;
use rune_ast::{Block, Expr, FnDecl, FnBody, Span};
use crate::eval_host::Flow; 
use crate::eval_pattern::bind_pattern;

#[derive(Clone)]
pub enum FnBodyKind {
    Block(Block),
    Expr(Expr),
}

pub fn eval_user_fn<H: EvalHost>(
    host: &mut H,
    f: &FnDecl,
    args: &[Value],
    _span: Span,
) -> Value {
    // Открываем новый scope перед вызовом
    host.push_scope();

    for (param, arg) in f.params.iter().zip(args.iter()) {
        bind_pattern(host, &param.pattern, arg);
    }

    // EXECUTE BODY 
    let result = match &f.body {
        FnBody::Expr(expr) => {
            // fn x() = expr
            Value::from(eval_expr(host, expr))
        }

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

    // Закрыть scope функции
    host.pop_scope();

    result
}
