use rune_ast::Type;

use super::Value;

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self { Value::Number(n) => Some(*n), _ => None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self { Value::Bool(b) => Some(*b), _ => None }
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

            _ => panic!("Cannot assign field '{}' on non-struct value {:?}", field, self),
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
            Value::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn matches_type(&self, ty: &Type) -> bool {
        match (self, ty) {
                        (Value::Number(_), Type::Simple(t))
                    if t == "i32" || t == "f32" || t == "f64" || t == "number"
                    => true,

            (Value::Bool(_), Type::Simple(t)) if t == "bool" => true,

            (Value::String(_), Type::Simple(t)) if t == "string" => true,

            (Value::Unit, Type::Unit) => true,

                        (Value::StructInstance { name, .. }, Type::Simple(t))
                if name == t => true,

                        (Value::EnumInstance { enum_name, .. }, Type::Simple(t))
                if enum_name == t => true,

            // Tuple matching can be added once the AST exposes Type::Tuple.
            // (Value::Tuple(vals), Type::Tuple(types))
            //     if vals.len() == types.len() => true,

            // Generic type arguments are not enforced by the runtime yet.
            (_, Type::Generic(_, _)) => true,

            _ => false,
        }
    }
}
