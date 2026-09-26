use super::super::*;

impl TintVM {
    pub(super) fn host_apply_compound(
        &mut self,
        left: &EvalValue,
        op: &str,
        right: &EvalValue,
    ) -> EvalValue {
        match (left, op, right) {
            (EvalValue::Number(a), "+=", EvalValue::Number(b)) => EvalValue::Number(a + b),

            (EvalValue::Number(a), "-=", EvalValue::Number(b)) => EvalValue::Number(a - b),

            (EvalValue::Number(a), "*=", EvalValue::Number(b)) => EvalValue::Number(a * b),

            (EvalValue::Number(a), "/=", EvalValue::Number(b)) => EvalValue::Number(a / b),

            // Strings: s += "text"
            (EvalValue::String(a), "+=", EvalValue::String(b)) => {
                EvalValue::String(format!("{}{}", a, b))
            }

            // Lists: list += elem
            (EvalValue::List(a), "+=", v) => {
                let mut out = a.clone();
                out.push(v.clone());
                EvalValue::List(out)
            }

            _ => EvalValue::Unit,
        }
    }
}
