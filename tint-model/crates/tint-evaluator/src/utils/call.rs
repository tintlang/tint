use crate::errors::{EvalError, EvalResult};
use crate::{value::Value, EvalHost};
use tint_ast::Span;

pub fn call_builtin<H: EvalHost>(
    _host: &mut H,
    name: &str,
    args: &[Value],
    span: Span,
) -> EvalResult<Value> {
    match name {
        // `log` is an alias for `print`.
        "print" | "log" => {
            for a in args {
                let line = format!("{:?}", a);
                println!("{}", line);
                crate::output::write_line(&line);
            }
            Ok(Value::Unit)
        }

        // `debug` is an alias for `dbg`.
        "dbg" | "debug" => {
            let line = format!("DBG: {:?}", args);
            println!("{}", line);
            crate::output::write_line(&line);
            Ok(Value::Unit)
        }

        // The browser has no separate error channel, so capture these lines too.
        "error" => {
            for a in args {
                let line = format!("ERROR: {:?}", a);
                eprintln!("{}", line);
                crate::output::write_line(&line);
            }
            Ok(Value::Unit)
        }

        "sqrt" => {
            let x = args
                .get(0)
                .and_then(|v| v.as_number())
                .ok_or(EvalError::InvalidOp {
                    msg: "sqrt expects 1 number".into(),
                    span,
                })?;

            Ok(Value::Number(x.sqrt()))
        }

        _ => {
            // Don\'t fall back - let the caller handle dispatch
            Err(EvalError::InvalidOp {
                msg: format!("Unknown builtin function: {}", name),
                span,
            })
        }
    }
}
