use crate::value::Value;
use tint_ast::*;

pub fn match_pattern(pat: &Pattern, val: &Value) -> bool {
    match pat {
        Pattern::Wildcard(_) => true,

        Pattern::Number(n, _) => matches!(val,
            Value::Number(v) if *n == v.to_string()
        ),

        Pattern::String(s, _) => matches!(val,
            Value::String(v) if s == v
        ),

        Pattern::Ident(_, _) => true, // binding always matches

        Pattern::Tuple(pats, _) => match val {
            Value::Tuple(vals) if vals.len() == pats.len() => {
                pats.iter().zip(vals).all(|(p, v)| match_pattern(p, v))
            }
            _ => false,
        },

        Pattern::Mut { inner, .. } => match_pattern(inner, val),

        Pattern::Group { fields, .. } => match val {
            Value::Map(map) => {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if !map.contains_key(field) {
                                return false;
                            }
                        }
                        PatternField::Assign { field, pat, .. } => match map.get(field) {
                            Some(v) => {
                                if !match_pattern(pat, v) {
                                    return false;
                                }
                            }
                            None => return false,
                        },
                        PatternField::Rest(_) => { /* ignore */ }
                    }
                }
                true
            }

            _ => false,
        },

        Pattern::Struct { name, fields, .. } => match val {
            Value::StructInstance {
                name: inst,
                fields: inst_fields,
            } if name == inst => {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if !inst_fields.iter().any(|(n, _)| n == field) {
                                return false;
                            }
                        }

                        PatternField::Assign { field, pat, .. } => {
                            match inst_fields.iter().find(|(n, _)| n == field) {
                                Some((_, v)) => {
                                    if !match_pattern(pat, v) {
                                        return false;
                                    }
                                }
                                None => return false,
                            }
                        }

                        PatternField::Rest(_) => continue,
                    }
                }
                true
            }
            _ => false,
        },

        Pattern::Map { fields, .. } => match val {
            Value::Map(map) => {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            if !map.contains_key(field) {
                                return false;
                            }
                        }
                        PatternField::Assign { field, pat, .. } => match map.get(field) {
                            Some(v) => {
                                if !match_pattern(pat, v) {
                                    return false;
                                }
                            }
                            None => return false,
                        },
                        PatternField::Rest(_) => { /* ignore */ }
                    }
                }

                true
            }

            _ => false,
        },

        Pattern::Variant { name, args, .. } => match val {
            Value::EnumInstance {
                enum_name,
                variant,
                args: inst_args,
            } if variant == name => {
                if args.len() != inst_args.len() {
                    return false;
                }

                args.iter()
                    .zip(inst_args.iter())
                    .all(|(p, v)| match_pattern(p, v))
            }
            _ => false,
        },
        Pattern::Typed { pat, ty, .. } => {
            if !val.matches_type(ty) {
                return false;
            }
            match_pattern(pat, val)
        }
    }
}
