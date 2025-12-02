// rune-evaluator/call.rs

use crate::{value::Value, EvalHost};
use crate::errors::{EvalError, EvalResult};
use rune_ast::Span;

pub fn call_builtin<H: EvalHost>(
    host: &mut H,
    name: &str,
    args: &[Value],
    span: Span,
) -> EvalResult<Value> {
    match name {
        "print" => {
            for a in args {
                println!("{:?}", a);
            }
            Ok(Value::Unit)
        }

        "dbg" => {
            println!("DBG: {:?}", args);
            Ok(Value::Unit)
        }

        "sqrt" => {
            let x = args.get(0).and_then(|v| v.as_number())
                .ok_or(EvalError::InvalidOp {
                    msg: "sqrt expects 1 number".into(),
                    span,
                })?;

            Ok(Value::Number(x.sqrt()))
        }

        _ => {
            // теперь работает!
            Ok(host.call_user_fn(name, args, span))
        }
    }
}
