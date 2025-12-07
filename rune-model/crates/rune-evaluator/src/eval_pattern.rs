use rune_ast::*;
use crate::{value::Value, EvalHost};

pub fn bind_pattern<H: EvalHost>(
    host: &mut H,
    pat: &Pattern,
    val: &Value,
) {
    match pat {
        Pattern::Ident(name, _) => {
            host.define_var(name, val.clone());
        }

        Pattern::Wildcard(_) => {
            // ignore
        }

        Pattern::Number(_, _) | Pattern::String(_, _) => {
            // literal patterns shouldn't appear in bindings normally
        }

        Pattern::Tuple(items, _) => {
            if let Value::Tuple(vals) = val {
                for (subpat, subval) in items.iter().zip(vals) {
                    bind_pattern(host, subpat, subval);
                }
            } else {
                panic!("Tuple pattern on non-tuple value");
            }
        }

        Pattern::Mut { inner, .. } => {
            bind_pattern(host, inner, val);
        }

        // ----------------------------------------------
        // NEW: anonymous map/group pattern → {a, b: pat}
        // ----------------------------------------------
        Pattern::Group { fields, .. } => {
            if let Value::Map(map) = val {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if let Some(v) = map.get(field) {
                                host.define_var(field, v.clone());
                            }
                        }

                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) = map.get(field) {
                                bind_pattern(host, pat, v);
                            }
                        }

                        PatternField::Rest(_) => { /* ignore */ }
                    }
                }
            } else {
                panic!("Group pattern applied to non-map value");
            }
        }

        // ----------------------------------------------
        // map { a, b{2}, c: pat }
        // ----------------------------------------------
        Pattern::Map { fields, .. } => {
            if let Value::Map(map) = val {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if let Some(v) = map.get(field) {
                                host.define_var(field, v.clone());
                            }
                        }

                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) = map.get(field) {
                                bind_pattern(host, pat, v);
                            }
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            } else {
                panic!("Map pattern applied to non-map value");
            }
        }

        // ----------------------------------------------
        // User { id, name, age }
        // ----------------------------------------------
        Pattern::Struct { name, fields, .. } => {
            if let Value::StructInstance { name: vname, fields: vfields } = val {
                if vname != name {
                    panic!("Struct pattern expects `{}`, got `{}`", name, vname);
                }

                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if let Some(v) = vfields.iter()
                                .find(|(k,_)| k == field)
                                .map(|(_,v)| v.clone())
                            {
                                host.define_var(field, v);
                            }
                        }

                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) = vfields.iter()
                                .find(|(k,_)| k == field)
                                .map(|(_,v)| v.clone())
                            {
                                bind_pattern(host, pat, &v);
                            }
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            } else {
                panic!("Struct pattern on non-struct value");
            }
        }

        // ----------------------------------------------
        // Some(x), Error(msg, code)
        // ----------------------------------------------
        Pattern::Variant { name, args, .. } => {
            if let Value::EnumInstance { variant, args: v_args, .. } = val {
                if variant != name {
                    panic!("Variant mismatch: expected `{}`, got `{}`", name, variant);
                }

                for (subpat, subval) in args.iter().zip(v_args.iter()) {
                    bind_pattern(host, subpat, subval);
                }

            } else {
                panic!("Variant pattern applied to non-enum value");
            }
        }

        // ----------------------------------------------
        // typed pattern: x: i32
        // ----------------------------------------------
        Pattern::Typed { pat, ty, .. } => {
            if !val.matches_type(ty) {
                panic!("Type mismatch in typed pattern");
            }
            bind_pattern(host, pat, val);
        }
    }
}
