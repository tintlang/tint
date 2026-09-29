use tint_ast::Span;

use super::HostVM;
use crate::{
    errors::{EvalError, EvalResult},
    eval_fn,
    eval_host::{EvalHost, Flow},
    value::Value,
};

pub(super) fn call_fn(
    host: &mut HostVM,
    name: &str,
    args: &[Value],
    span: Span,
) -> EvalResult<Value> {
    if let Some(Value::HostFunction(function)) = host.modules.get(name) {
        return Ok(function(args.to_vec()));
    }

    if let Some(function) = host.functions.get(name).cloned() {
        // Clone the declaration so evaluating the call can mutably borrow the host.
        return Ok(eval_fn::eval_user_fn(host, &function, args, span));
    }

    Err(EvalError::InvalidOp {
        msg: format!("Unknown function '{}'", name),
        span,
    })
}

pub(super) fn call_user_fn(
    host: &mut HostVM,
    name: &str,
    args: &[Value],
    span: Span,
) -> EvalResult<Value> {
    let Some(function) = host.functions.get(name).cloned() else {
        return Err(EvalError::InvalidOp {
            msg: format!("Unknown user-fn '{}'", name),
            span,
        });
    };

    Ok(eval_fn::eval_user_fn(host, &function, args, span))
}

pub(super) fn call_value(host: &mut HostVM, value: Value, args: &[Value], span: Span) -> Value {
    match value {
        Value::HostFunction(function) => function(args.to_vec()),
        Value::Function {
            params, body, env, ..
        } => {
            host.push_scope();
            host.env.extend_from(&env);

            for (param, arg) in params.iter().zip(args) {
                host.define_var(param, arg.clone());
            }

            let result = match body {
                eval_fn::FnBodyKind::Block(block) => match host.eval_block(&block) {
                    Flow::Value(value) | Flow::Return(value) => value,
                    Flow::Break | Flow::Continue => Value::Unit,
                    Flow::Propagate(value) => value,
                },
                eval_fn::FnBodyKind::Expr(expr) => host.eval_expr(&expr),
            };

            host.pop_scope();
            result
        }
        Value::Lambda {
            params,
            body,
            closure,
        } => {
            host.push_scope();
            host.env.extend_from(&closure);

            for (param, arg) in params.iter().zip(args) {
                host.define_var(param, arg.clone());
            }

            let result = host.eval_expr(&body);
            host.pop_scope();
            result
        }
        _ => panic!("Value is not callable at {:?}", span),
    }
}
