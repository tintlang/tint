use tint_ast::Type;

use super::Value;

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::I32(n) => Some(f64::from(*n)),
            Value::I64(n) => Some(*n as f64),
            Value::U32(n) => Some(f64::from(*n)),
            Value::U64(n) => Some(*n as f64),
            Value::U8(n) => Some(f64::from(*n)),
            Value::F32(n) => Some(f64::from(*n)),
            Value::F64(n) => Some(*n),
            _ => None,
        }
    }

    pub fn cast_numeric(&self, ty: &Type) -> Result<Value, String> {
        let value = self
            .as_number()
            .ok_or_else(|| "expected a numeric value".to_owned())?;
        match ty {
            Type::Simple(name) if name == "i32" => {
                integral_cast(value, i32::MIN as f64, i32::MAX as f64).map(|n| Value::I32(n as i32))
            }
            Type::Simple(name) if name == "i64" => {
                integral_cast(value, i64::MIN as f64, i64::MAX as f64).map(|n| Value::I64(n as i64))
            }
            Type::Simple(name) if name == "u32" => {
                integral_cast(value, 0.0, u32::MAX as f64).map(|n| Value::U32(n as u32))
            }
            Type::Simple(name) if name == "u64" => {
                integral_cast(value, 0.0, u64::MAX as f64).map(|n| Value::U64(n as u64))
            }
            Type::Simple(name) if name == "u8" => {
                if !value.is_finite() || value.fract() != 0.0 || !(0.0..=255.0).contains(&value) {
                    return Err(format!(
                        "cannot convert {value} to u8: value is out of range or not an integer"
                    ));
                }
                Ok(Value::U8(value as u8))
            }
            Type::Simple(name) if name == "f32" => {
                if !value.is_finite() || value > f32::MAX as f64 || value < -(f32::MAX as f64) {
                    return Err(format!(
                        "cannot convert {value} to f32: value is out of range"
                    ));
                }
                Ok(Value::F32(value as f32))
            }
            Type::Simple(name) if name == "f64" || name == "number" => Ok(Value::F64(value)),
            _ => Err("`as` requires a numeric target type".into()),
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Set struct field: obj.field = value
    pub fn set_field(&mut self, field: &str, new_value: Value) {
        match self {
            Value::StructInstance { fields, .. } => {
                for (name, val) in fields.iter_mut() {
                    if name == field {
                        *val = new_value;
                        return;
                    }
                }
                panic!("Field '{}' not found in struct", field);
            }

            Value::Map(map) => {
                map.insert(field.to_string(), new_value);
            }

            _ => panic!(
                "Cannot assign field '{}' on non-struct value {:?}",
                field, self
            ),
        }
    }

    /// Set array/list index: arr[i] = value
    pub fn set_index(&mut self, index: i64, new_value: Value) {
        match self {
            Value::List(items) => {
                let idx = index as usize;
                if idx >= items.len() {
                    panic!("Index {} out of bounds", index);
                }
                items[idx] = new_value;
            }

            Value::Tuple(items) => {
                let idx = index as usize;
                if idx >= items.len() {
                    panic!("Tuple index {} out of bounds", index);
                }
                items[idx] = new_value;
            }

            _ => panic!("Cannot index-assign on non-list value {:?}", self),
        }
    }

    pub fn force_bool(&self) -> bool {
        self.as_bool().unwrap_or(false)
    }
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Number(n) => *n as i64,
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn matches_type(&self, ty: &Type) -> bool {
        match (self, ty) {
            (Value::Number(_), Type::Simple(t))
                if t == "i32"
                    || t == "i64"
                    || t == "u8"
                    || t == "u32"
                    || t == "u64"
                    || t == "f32"
                    || t == "f64"
                    || t == "number" =>
            {
                true
            }

            (Value::U8(_), Type::Simple(t)) if t == "u8" || t == "number" => true,
            (Value::I32(_), Type::Simple(t)) if t == "i32" || t == "number" => true,
            (Value::I64(_), Type::Simple(t)) if t == "i64" || t == "number" => true,
            (Value::U32(_), Type::Simple(t)) if t == "u32" || t == "number" => true,
            (Value::U64(_), Type::Simple(t)) if t == "u64" || t == "number" => true,
            (Value::F32(_), Type::Simple(t)) if t == "f32" || t == "number" => true,
            (Value::F64(_), Type::Simple(t)) if t == "f64" || t == "number" => true,

            (Value::Bool(_), Type::Simple(t)) if t == "bool" => true,

            (Value::String(_), Type::Simple(t)) if t == "string" => true,

            (Value::Unit, Type::Unit) => true,

            (Value::StructInstance { name, .. }, Type::Simple(t)) if name == t => true,

            (Value::EnumInstance { enum_name, .. }, Type::Simple(t)) if enum_name == t => true,

            // Tuple matching can be added once the AST exposes Type::Tuple.
            // (Value::Tuple(vals), Type::Tuple(types))
            //     if vals.len() == types.len() => true,

            // Generic type arguments are not enforced by the runtime yet.
            (_, Type::Generic(_, _)) => true,
            // Function signatures are compile-time contracts. The evaluator
            // already receives a callable Value here, so accepting the
            // value is the correct runtime check.
            (_, Type::Function { .. }) => matches!(
                self,
                Value::Lambda { .. } | Value::Function { .. } | Value::HostFunction(_) | Value::Callback(_)
            ),

            _ => false,
        }
    }
}

fn integral_cast(value: f64, min: f64, max: f64) -> Result<f64, String> {
    if !value.is_finite() || value.fract() != 0.0 || value < min || value > max {
        Err(format!(
            "cannot convert {value}: value is out of range or not an integer"
        ))
    } else {
        Ok(value)
    }
}
