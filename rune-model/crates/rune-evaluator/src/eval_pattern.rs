use rune_ast::Pattern;
use rune_ast::PatternField;
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
            // нельзя “bind”, это только для match;  
            // но в параметрах функции такие не должны появляться
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
            // mut в паттерне просто аннотация
            bind_pattern(host, inner, val);
        }

        Pattern::Map { fields, .. } => {
            if let Value::Map(map) = val {
                for f in fields {
                    match f {
                        // shorthand: map { x }
                        PatternField::Shorthand { field, .. } => {
                            if let Some(v) = map.get(field) {
                                host.define_var(field, v.clone());
                            }
                        }

                        // assign: map { x: pat }
                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) = map.get(field) {
                                bind_pattern(host, pat, v);
                            }
                        }

                        // rest `..` в map-паттерне сейчас игнорируем
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
                            if let Some(v) =
                                vfields.iter().find(|(k,_)| k == field).map(|(_,v)| v.clone())
                            {
                                host.define_var(field, v);
                            }
                        }

                        PatternField::Assign { field, pat, .. } => {
                            if let Some(v) =
                                vfields.iter().find(|(k,_)| k == field).map(|(_,v)| v.clone())
                            {
                                bind_pattern(host, pat, &v);
                            }
                        }

                        PatternField::Rest(_) => {
                            // ignore rest fields
                        }
                    }
                }
            } else {
                panic!("Struct pattern on non-struct value");
            }
        }

       Pattern::Variant { name, args, .. } => {
        if let Value::EnumInstance { enum_name: _, variant: vname, args: vargs } = val {
            
            // variant must match
            if vname != name {
                panic!("Variant `{}` expected, got `{}`", name, vname);
            }

            // bind payload
            for (subpat, subval) in args.iter().zip(vargs.iter()) {
                bind_pattern(host, subpat, subval);
            }

        } else {
            panic!("Variant pattern on non-enum value");
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
