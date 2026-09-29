//! Option / Result methods beyond the core set in `calls.rs`.

use super::super::*;

fn invalid(msg: impl Into<String>, span: Span) -> tint_evaluator::errors::EvalError {
    tint_evaluator::errors::EvalError::InvalidOp { msg: msg.into(), span }
}

fn adt(name: &str, variant: &str, args: Vec<EvalValue>) -> EvalValue {
    EvalValue::EnumInstance { enum_name: name.into(), variant: variant.into(), args }
}

fn none() -> EvalValue {
    adt("Option", "None", Vec::new())
}

fn some(v: EvalValue) -> EvalValue {
    adt("Option", "Some", vec![v])
}

impl TintVM {
    /// Handles the methods not in `calls.rs`. `None` when `method` is not one of
    /// them (or the receiver is not an Option/Result).
    pub(super) fn option_result_method(
        &mut self,
        receiver: &EvalValue,
        method: &str,
        args: &[EvalValue],
        span: Span,
    ) -> Option<EvalResult<EvalValue>> {
        let EvalValue::EnumInstance { enum_name, variant, args: values } = receiver else {
            return None;
        };
        let is_option = enum_name == "Option";
        if !is_option && enum_name != "Result" {
            return None;
        }
        let ok = if is_option { variant == "Some" } else { variant == "Ok" };
        let payload = values.first().cloned().unwrap_or(EvalValue::Unit);
        let arity = |want: usize| -> Option<EvalResult<EvalValue>> {
            (args.len() != want)
                .then(|| Err(invalid(format!("{method} expects {want} argument(s)"), span)))
        };
        // The value a fallback closure receives: nothing for Option, the error for Result.
        let fail_args = if is_option { Vec::new() } else { vec![payload.clone()] };

        let out = match (is_option, method) {
            (true, "is_some_and") | (false, "is_ok_and") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok {
                    EvalValue::Bool(self.host_call_value(args[0].clone(), &[payload], span).force_bool())
                } else {
                    EvalValue::Bool(false)
                }
            }
            (false, "is_err_and") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok {
                    EvalValue::Bool(false)
                } else {
                    EvalValue::Bool(self.host_call_value(args[0].clone(), &[payload], span).force_bool())
                }
            }
            (_, "unwrap_or_else") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok { payload } else { self.host_call_value(args[0].clone(), &fail_args, span) }
            }
            (_, "map_or") => {
                if let Some(e) = arity(2) { return Some(e) }
                if ok {
                    self.host_call_value(args[1].clone(), &[payload], span)
                } else {
                    args[0].clone()
                }
            }
            (true, "filter") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok && self.host_call_value(args[0].clone(), &[payload], span).force_bool() {
                    receiver.clone()
                } else {
                    none()
                }
            }
            (true, "or") | (false, "or") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok { receiver.clone() } else { args[0].clone() }
            }
            (true, "or_else") | (false, "or_else") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok { receiver.clone() } else { self.host_call_value(args[0].clone(), &fail_args, span) }
            }
            (true, "ok_or") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok { adt("Result", "Ok", vec![payload]) } else { adt("Result", "Err", vec![args[0].clone()]) }
            }
            (true, "ok_or_else") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok {
                    adt("Result", "Ok", vec![payload])
                } else {
                    let e = self.host_call_value(args[0].clone(), &[], span);
                    adt("Result", "Err", vec![e])
                }
            }
            (false, "map_err") => {
                if let Some(e) = arity(1) { return Some(e) }
                if ok {
                    receiver.clone()
                } else {
                    let e = self.host_call_value(args[0].clone(), &[payload], span);
                    adt("Result", "Err", vec![e])
                }
            }
            (false, "ok") => {
                if let Some(e) = arity(0) { return Some(e) }
                if ok { some(payload) } else { none() }
            }
            (false, "err") => {
                if let Some(e) = arity(0) { return Some(e) }
                if ok { none() } else { some(payload) }
            }
            (false, "unwrap_err") | (false, "expect_err") => {
                if let Some(e) = arity(if method == "expect_err" { 1 } else { 0 }) { return Some(e) }
                if ok {
                    return Some(Err(invalid(format!("{method} called on an Ok value"), span)));
                }
                payload
            }
            _ => return None,
        };
        Some(Ok(out))
    }
}
