use tint_ast::{Pattern, PatternField};

use super::HostVM;
use crate::{eval_host::EvalHost, value::Value};

pub(super) fn bind_pattern(host: &mut HostVM, pattern: &Pattern, value: &Value) -> bool {
    match pattern {
        Pattern::Wildcard(_) => true,
        Pattern::Ident(name, _) => {
            host.define_var(name, value.clone());
            true
        }
        Pattern::Number(number, _) => matches!(value, Value::Number(v) if v.to_string() == *number),
        Pattern::String(string, _) => matches!(value, Value::String(v) if v == string),
        Pattern::Mut { inner, .. } => host.match_pattern(value, inner),
        Pattern::Group { fields, .. } | Pattern::Map { fields, .. } => {
            let Value::Map(map) = value else {
                return false;
            };

            for field in fields {
                match field {
                    PatternField::Shorthand { field, .. } => {
                        let Some(value) = map.get(field).cloned() else {
                            return false;
                        };
                        host.define_var(field, value);
                    }
                    PatternField::Assign { field, pat, .. } => {
                        let Some(value) = map.get(field) else {
                            return false;
                        };
                        if !bind_pattern(host, pat, value) {
                            return false;
                        }
                    }
                    PatternField::Rest(_) => {}
                }
            }
            true
        }
        Pattern::Tuple(patterns, _) => {
            let Value::Tuple(values) = value else {
                return false;
            };
            patterns.len() == values.len()
                && patterns
                    .iter()
                    .zip(values)
                    .all(|(pattern, value)| bind_pattern(host, pattern, value))
        }
        Pattern::Struct { name, fields, .. } => {
            let Value::StructInstance {
                name: value_name,
                fields: value_fields,
            } = value
            else {
                return false;
            };
            if value_name != name {
                return false;
            }

            for field in fields {
                match field {
                    PatternField::Shorthand { field, .. } => {
                        let Some(value) = find_struct_field(value_fields, field).cloned() else {
                            return false;
                        };
                        host.define_var(field, value);
                    }
                    PatternField::Assign { field, pat, .. } => {
                        let Some(value) = find_struct_field(value_fields, field) else {
                            return false;
                        };
                        if !bind_pattern(host, pat, value) {
                            return false;
                        }
                    }
                    PatternField::Rest(_) => {}
                }
            }
            true
        }
        Pattern::Variant { name, args, .. } => {
            let Value::EnumInstance {
                variant,
                args: values,
                ..
            } = value
            else {
                return false;
            };
            variant == name
                && args.len() == values.len()
                && args
                    .iter()
                    .zip(values)
                    .all(|(pattern, value)| bind_pattern(host, pattern, value))
        }
        Pattern::Typed { pat, ty, .. } => value.matches_type(ty) && bind_pattern(host, pat, value),
    }
}

pub(super) fn match_pattern(host: &mut HostVM, value: &Value, pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard(_) | Pattern::Ident(_, _) => true,
        Pattern::Number(number, _) => matches!(value, Value::Number(v) if v.to_string() == *number),
        Pattern::String(string, _) => matches!(value, Value::String(v) if v == string),
        Pattern::Mut { inner, .. } => bind_pattern(host, inner, value),
        Pattern::Group { fields, .. } => match_map_with_bindings(host, value, fields),
        Pattern::Tuple(patterns, _) => {
            let Value::Tuple(values) = value else {
                return false;
            };
            patterns.len() == values.len()
                && patterns
                    .iter()
                    .zip(values)
                    .all(|(pattern, value)| match_pattern(host, value, pattern))
        }
        Pattern::Struct { name, fields, .. } => {
            let Value::StructInstance {
                name: value_name,
                fields: value_fields,
            } = value
            else {
                return false;
            };
            value_name == name
                && fields
                    .iter()
                    .all(|field| match_struct_field(host, value_fields, field))
        }
        Pattern::Map { fields, .. } => {
            let Value::Map(map) = value else {
                return false;
            };
            fields.iter().all(|field| match_map_field(host, map, field))
        }
        Pattern::Variant { name, args, .. } => {
            let Value::EnumInstance {
                variant,
                args: values,
                ..
            } = value
            else {
                return false;
            };
            variant == name
                && args.len() == values.len()
                && args
                    .iter()
                    .zip(values)
                    .all(|(pattern, value)| match_pattern(host, value, pattern))
        }
        Pattern::Typed { pat, ty, .. } => value.matches_type(ty) && match_pattern(host, value, pat),
    }
}

fn match_map_with_bindings(host: &mut HostVM, value: &Value, fields: &[PatternField]) -> bool {
    let Value::Map(map) = value else {
        return false;
    };

    for field in fields {
        match field {
            PatternField::Shorthand { field, .. } => {
                let Some(value) = map.get(field).cloned() else {
                    return false;
                };
                host.define_var(field, value);
            }
            PatternField::Assign { field, pat, .. } => {
                let Some(value) = map.get(field) else {
                    return false;
                };
                if !bind_pattern(host, pat, value) {
                    return false;
                }
            }
            PatternField::Rest(_) => {}
        }
    }
    true
}

fn match_struct_field(
    host: &mut HostVM,
    fields: &[(String, Value)],
    pattern: &PatternField,
) -> bool {
    match pattern {
        PatternField::Shorthand { field, .. } => find_struct_field(fields, field).is_some(),
        PatternField::Assign { field, pat, .. } => {
            find_struct_field(fields, field).is_some_and(|value| match_pattern(host, value, pat))
        }
        PatternField::Rest(_) => true,
    }
}

fn match_map_field(
    host: &mut HostVM,
    map: &std::collections::HashMap<String, Value>,
    pattern: &PatternField,
) -> bool {
    match pattern {
        PatternField::Shorthand { field, .. } => map.contains_key(field),
        PatternField::Assign { field, pat, .. } => map
            .get(field)
            .is_some_and(|value| match_pattern(host, value, pat)),
        PatternField::Rest(_) => true,
    }
}

fn find_struct_field<'a>(fields: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value)
}
