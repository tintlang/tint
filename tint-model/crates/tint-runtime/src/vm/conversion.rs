use super::*;

impl TintVM {
    pub(super) fn eval_value_to_ir_value(v: EvalValue) -> IrValue {
        match v {
            EvalValue::Number(n) => IrValue::Number(n),
            EvalValue::I32(n) => IrValue::Number(f64::from(n)),
            EvalValue::I64(n) => IrValue::Number(n as f64),
            EvalValue::U32(n) => IrValue::Number(f64::from(n)),
            EvalValue::U64(n) => IrValue::Number(n as f64),
            EvalValue::U8(n) => IrValue::Number(f64::from(n)),
            EvalValue::F32(n) => IrValue::Number(f64::from(n)),
            EvalValue::F64(n) => IrValue::Number(n),
            EvalValue::String(s) => IrValue::String(s),
            EvalValue::Bool(b) => IrValue::Bool(b),
            EvalValue::Unit => IrValue::Unit,
            EvalValue::Propagate(value) => Self::eval_value_to_ir_value(*value),
            EvalValue::Tuple(items) => IrValue::Tuple(
                items
                    .into_iter()
                    .map(Self::eval_value_to_ir_value)
                    .collect(),
            ),
            EvalValue::List(items) => IrValue::List(
                items
                    .into_iter()
                    .map(Self::eval_value_to_ir_value)
                    .collect(),
            ),
            EvalValue::EnumInstance {
                enum_name,
                variant,
                args,
            } => IrValue::EnumInstance {
                enum_name,
                variant,
                // `tint_evaluator::Value::EnumInstance` is still
                // positional-only (`args: Vec<Value>`, no field names) --
                // only the IR side's `Value::EnumInstance` was changed to
                // carry names (see its doc comment in tint-ir/src/ir.rs),
                // so there's no real name to carry across this boundary.
                // Synthesizing a placeholder name would be worse than
                // admitting there isn't one: it would look like a real
                // field name to anything matching on it later. A
                // brace-style variant pattern (`A { x } => ..`) can't
                // match a value that arrived via this conversion either
                // way, since the tree-walking evaluator that produced it
                // never captured the name to begin with.
                fields: args
                    .into_iter()
                    .map(|v| (String::new(), Self::eval_value_to_ir_value(v)))
                    .collect(),
            },
            EvalValue::Map(map) => IrValue::Map(
                map.into_iter()
                    .map(|(k, v)| (k, Self::eval_value_to_ir_value(v)))
                    .collect(),
            ),
            EvalValue::StructInstance { name, fields } => IrValue::StructInstance {
                name,
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, Self::eval_value_to_ir_value(v)))
                    .collect(),
            },
            // Lambdas/functions/host-functions/namespaces have no IR-value
            // representation; the IR VM doesn't support calling through them.
            EvalValue::Lambda { .. }
            | EvalValue::Function { .. }
            | EvalValue::HostFunction(_)
            | EvalValue::Callback(_)
            | EvalValue::Namespace { .. } => IrValue::Unit,
        }
    }

    pub(super) fn ir_value_to_eval_value(v: IrValue) -> EvalValue {
        match v {
            IrValue::Number(n) => EvalValue::Number(n),
            IrValue::String(s) => EvalValue::String(s),
            IrValue::Bool(b) => EvalValue::Bool(b),
            IrValue::Unit => EvalValue::Unit,
            IrValue::Tuple(items) => EvalValue::Tuple(
                items
                    .into_iter()
                    .map(Self::ir_value_to_eval_value)
                    .collect(),
            ),
            IrValue::List(items) => EvalValue::List(
                items
                    .into_iter()
                    .map(Self::ir_value_to_eval_value)
                    .collect(),
            ),
            IrValue::EnumInstance {
                enum_name,
                variant,
                fields,
            } => EvalValue::EnumInstance {
                enum_name,
                variant,
                // The reverse of the conversion above: the target type
                // here (`tint_evaluator::Value::EnumInstance`) is still
                // positional-only, so the field names this IR value
                // carries are dropped, not because they don't matter but
                // because there's nowhere on the other side to put them.
                args: fields
                    .into_iter()
                    .map(|(_, v)| Self::ir_value_to_eval_value(v))
                    .collect(),
            },
            IrValue::Map(map) => EvalValue::Map(
                map.into_iter()
                    .map(|(k, v)| (k, Self::ir_value_to_eval_value(v)))
                    .collect(),
            ),
            IrValue::StructInstance { name, fields } => EvalValue::StructInstance {
                name,
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, Self::ir_value_to_eval_value(v)))
                    .collect(),
            },
        }
    }
}
