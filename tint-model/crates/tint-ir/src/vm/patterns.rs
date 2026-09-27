use super::*;
use tint_ast::{Pattern, PatternField, Type};

impl<'a> IrVM<'a> {
    // Shared by `Pattern::Struct` against either a `StructInstance`'s or an
    // `EnumInstance`'s named fields (see the doc comment where this is
    // called): walks the pattern's field list against a `(name, value)`
    // slice, binding/recursing for `Shorthand`/`Assign` and skipping `Rest`.
    pub(super) fn match_named_fields(
        &self,
        pattern_fields: &[PatternField],
        value_fields: &[(String, Value)],
        bindings: &mut std::collections::HashMap<String, Value>,
    ) -> bool {
        for field_pattern in pattern_fields {
            match field_pattern {
                PatternField::Shorthand { field, .. } => {
                    if let Some((_, v)) = value_fields.iter().find(|(k, _)| k == field) {
                        bindings.insert(field.clone(), v.clone());
                    } else {
                        return false;
                    }
                }
                PatternField::Assign { field, pat, .. } => {
                    if let Some((_, v)) = value_fields.iter().find(|(k, _)| k == field) {
                        if !self.pattern_matches(pat, v, bindings) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                PatternField::Rest(_) => {
                    // Rest pattern matches remaining fields
                }
            }
        }
        true
    }

    // Pattern matching helper functions
    pub(super) fn match_pattern(
        &self,
        pattern: &Pattern,
        value: &Value,
    ) -> Option<std::collections::HashMap<String, Value>> {
        let mut bindings = std::collections::HashMap::new();

        if self.pattern_matches(pattern, value, &mut bindings) {
            Some(bindings)
        } else {
            None
        }
    }

    pub(super) fn pattern_matches(
        &self,
        pattern: &Pattern,
        value: &Value,
        bindings: &mut std::collections::HashMap<String, Value>,
    ) -> bool {
        match pattern {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything without binding
                true
            }

            Pattern::Ident(name, _) => {
                // Ident matches anything and binds it
                bindings.insert(name.clone(), value.clone());
                true
            }

            Pattern::Number(num_str, _) => {
                // Match a specific number
                if let Ok(n) = num_str.parse::<f64>() {
                    if let Value::Number(v) = value {
                        *v == n
                    } else {
                        false
                    }
                } else {
                    false
                }
            }

            Pattern::String(s, _) => {
                // Match a specific string
                if let Value::String(v) = value {
                    v == s
                } else {
                    false
                }
            }

            Pattern::Tuple(patterns, _) => {
                // Match a tuple - recursively match each element
                if let Value::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return false;
                    }

                    for (pat, val) in patterns.iter().zip(values.iter()) {
                        if !self.pattern_matches(pat, val, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }

            Pattern::Struct { name, fields, .. } => {
                // `Name { field, .. }` -- matches a real struct BY NAME,
                // or (see the doc comment on `Value::EnumInstance`) a
                // struct-STYLE enum variant matched BY VARIANT name, since
                // the parser produces this same `Pattern::Struct` for both
                // shapes (`tint-parser`'s `parse_ident_based_pattern` can't
                // tell a struct name from a variant name apart at a bare
                // `Name {` pattern site) and both value kinds now carry
                // named fields the same way. `A { x } => ..` against
                // `E2::A { x { 50 } }` used to fall through to `_` every
                // time, matching neither branch below.
                match value {
                    Value::StructInstance {
                        name: struct_name,
                        fields: struct_fields,
                    } => {
                        if struct_name != name {
                            return false;
                        }
                        self.match_named_fields(fields, struct_fields, bindings)
                    }

                    Value::EnumInstance {
                        variant,
                        fields: enum_fields,
                        ..
                    } => {
                        if variant != name {
                            return false;
                        }
                        self.match_named_fields(fields, enum_fields, bindings)
                    }

                    _ => false,
                }
            }

            Pattern::Variant { name, args, .. } => {
                // `Name(a, b)` -- positional. `EnumInstance::fields` is
                // still stored in declaration order, so zipping against it
                // positionally (ignoring the names) is exactly the same
                // match this always did, just read off the renamed field.
                if let Value::EnumInstance {
                    variant,
                    fields: variant_fields,
                    ..
                } = value
                {
                    if variant != name {
                        return false;
                    }

                    if args.len() != variant_fields.len() {
                        return false;
                    }

                    for (pat, (_, val)) in args.iter().zip(variant_fields.iter()) {
                        if !self.pattern_matches(pat, val, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }

            // `x: i32 => ...` -- was previously caught by the `_ => true`
            // fallback below, meaning it matched UNCONDITIONALLY (no type
            // check at all) and bound NOTHING, so the arm body silently
            // read whatever the name already held from outer scope instead
            // of the actual scrutinee. Mirrors the already-correct
            // handling in tint-evaluator (runtime/host_vm/patterns.rs /
            // value/methods.rs's `matches_type`): check the runtime type,
            // then delegate to the inner pattern so the name is actually
            // bound to the scrutinee.
            Pattern::Typed { pat, ty, .. } => {
                Self::value_matches_type(value, ty) && self.pattern_matches(pat, value, bindings)
            }

            // Struct-like enum variants (`enum E { A { x } }`, matched as
            // `A { x } => ...`) used to be a known gap here -- `Name {`
            // always parses to `Pattern::Struct` (the parser can't tell a
            // struct name from a variant name apart at that bare syntax;
            // see tint-parser's `parse_ident_based_pattern`), and that only
            // ever matched `Value::StructInstance`, never
            // `Value::EnumInstance`. Fixed above: `Pattern::Struct` now
            // accepts either value kind (matching a `StructInstance` by
            // struct name or an `EnumInstance` by variant name), which
            // needed `Value::EnumInstance` to retain field NAMES instead of
            // a positional `Vec<Value>` -- see its doc comment in ir.rs.
            // This IR VM is the only place that needed fixing: this crate's
            // `Value` is private to it, so the tree-walking evaluator
            // (`tint-evaluator`, its own separate `Value`/pattern-matching
            // code) is untouched by this change and still has the
            // equivalent restriction on its own execution path.
            _ => true,
        }
    }

    // Runtime type check for `Pattern::Typed` (`x: i32`). Deliberately
    // mirrors tint-evaluator's `Value::matches_type` so the two execution
    // paths agree on what a type name means at runtime -- see that
    // function (runtime/value/methods.rs) for the reference behavior this
    // is kept in sync with.
    pub(super) fn value_matches_type(value: &Value, ty: &Type) -> bool {
        match ty {
            Type::Simple(t) => match value {
                Value::Number(_) => t == "i32" || t == "f32" || t == "f64" || t == "number",
                Value::Bool(_) => t == "bool",
                Value::String(_) => t == "string",
                Value::Unit => false,
                Value::StructInstance { name, .. } => name == t,
                Value::EnumInstance { enum_name, .. } => enum_name == t,
                _ => false,
            },
            Type::Unit => matches!(value, Value::Unit),
            // Generic type arguments aren't enforced by the runtime yet,
            // same as the evaluator.
            Type::Generic(_, _) => true,
            Type::Union(_) => true,
        }
    }
}
