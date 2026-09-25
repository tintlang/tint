use tint_ast::*;

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

        Pattern::Wildcard(_) => {}

        Pattern::Number(_, _) | Pattern::String(_, _) => {}

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
        // Anonymous group patterns bind fields from map-like values.
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

                        PatternField::Rest(_) => {}
                    }
                }
            } else {
                panic!("Group pattern applied to non-map value");
            }
        }
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
        Pattern::Typed { pat, ty, .. } => {
            if !val.matches_type(ty) {
                panic!("Type mismatch in typed pattern");
            }
            bind_pattern(host, pat, val);
        }
    }
}
