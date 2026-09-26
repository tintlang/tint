use super::super::*;

impl TintVM {
    pub(super) fn host_namespace_lookup(&mut self, _ns: EvalValue, item: &str) -> EvalValue {
        panic!("Namespaces not supported: {}", item);
    }

    pub(super) fn host_field_lookup(&mut self, obj: EvalValue, field: &str) -> EvalValue {
        match obj {
            EvalValue::StructInstance { fields, .. } => {
                for (name, value) in fields {
                    if name == field {
                        return value;
                    }
                }
                panic!("Field '{}' not found in struct", field);
            }
            EvalValue::Map(map) => map.get(field).cloned().unwrap_or(EvalValue::Unit),
            other => panic!(
                "Field lookup not supported: obj={:?}, field={}",
                other, field
            ),
        }
    }
    pub(super) fn host_index_lookup(&mut self, obj: EvalValue, idx: EvalValue) -> EvalValue {
        match (obj, idx) {
            (EvalValue::List(items), EvalValue::Number(n)) => {
                let index = n as usize;
                if index < items.len() {
                    items[index].clone()
                } else {
                    panic!("List index {} out of bounds (len={})", index, items.len());
                }
            }
            (EvalValue::Tuple(items), EvalValue::Number(n)) => {
                let index = n as usize;
                if index < items.len() {
                    items[index].clone()
                } else {
                    panic!("Tuple index {} out of bounds (len={})", index, items.len());
                }
            }
            (EvalValue::Map(map), EvalValue::String(key)) => {
                map.get(&key).cloned().unwrap_or(EvalValue::Unit)
            }
            (obj, idx) => panic!("Index lookup not supported: obj={:?}, idx={:?}", obj, idx),
        }
    }
}
