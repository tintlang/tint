use crate::value::Value;

pub(super) fn namespace_lookup(namespace: Value, item: &str) -> Value {
    match namespace {
        Value::Namespace { items, .. } => items
            .lookup(item)
            .unwrap_or_else(|| panic!("Unknown namespace item '{}'", item)),
        _ => panic!("Value is not a namespace"),
    }
}

pub(super) fn field_lookup(object: Value, field: &str) -> Value {
    match object {
        Value::StructInstance { fields, .. } => fields
            .into_iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("No field '{}' found", field)),
        _ => panic!("Value is not an object"),
    }
}

pub(super) fn index_lookup(object: Value, index: Value) -> Value {
    match (object, index.as_number()) {
        (Value::List(items), Some(index)) => items[index as usize].clone(),
        _ => panic!("Invalid indexing"),
    }
}

pub(super) fn apply_compound(left: &Value, op: &str, right: &Value) -> Value {
    match (left, op, right) {
        (Value::Number(a), "+=", Value::Number(b)) => Value::Number(a + b),
        (Value::Number(a), "-=", Value::Number(b)) => Value::Number(a - b),
        (Value::Number(a), "*=", Value::Number(b)) => Value::Number(a * b),
        (Value::Number(a), "/=", Value::Number(b)) => Value::Number(a / b),
        _ => Value::Unit,
    }
}
