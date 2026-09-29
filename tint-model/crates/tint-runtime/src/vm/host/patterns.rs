use super::super::*;

impl TintVM {
    pub(super) fn host_bind_pattern(&mut self, pat: &tint_ast::Pattern, value: &EvalValue) -> bool {
        use tint_ast::{Pattern, PatternField};

        match pat {
            Pattern::Wildcard(_) => true,

            Pattern::Ident(name, _) => {
                self.define_var(name, value.clone());
                true
            }

            Pattern::Mut { inner, .. } => self.host_bind_pattern(inner, value),

            Pattern::Number(s, _) => match value {
                EvalValue::Number(n) => n.to_string() == *s,
                _ => false,
            },

            Pattern::String(s, _) => match value {
                EvalValue::String(v) => v == s,
                _ => false,
            },

            // Group pattern: { a, b: pat }
            Pattern::Group { fields, .. } => match value {
                EvalValue::Map(map) => {
                    for f in fields {
                        match f {
                            // shorthand: { a }
                            PatternField::Shorthand { field, .. } => {
                                if let Some(v) = map.get(field) {
                                    self.define_var(field, v.clone());
                                } else {
                                    return false;
                                }
                            }

                            // assign: { a: pat }
                            PatternField::Assign { field, pat, .. } => {
                                if let Some(v) = map.get(field) {
                                    if !self.host_bind_pattern(pat, v) {
                                        return false;
                                    }
                                } else {
                                    return false;
                                }
                            }

                            PatternField::Rest(_) => { /* ignore */ }
                        }
                    }
                    true
                }
                _ => false,
            },

            // Tuple pattern: (a, b, c)
            Pattern::Tuple(p_items, _) => match value {
                EvalValue::Tuple(v_items) => {
                    if p_items.len() != v_items.len() {
                        return false;
                    }
                    for (p, v) in p_items.iter().zip(v_items.iter()) {
                        if !self.host_bind_pattern(p, v) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },

            // Struct pattern: User { x, y }
            Pattern::Struct { name, fields, .. } => match value {
                EvalValue::StructInstance {
                    name: vname,
                    fields: vfields,
                } => {
                    if vname != name {
                        return false;
                    }

                    for pf in fields {
                        match pf {
                            PatternField::Shorthand { field, .. } => {
                                // get value from struct
                                let Some(v) =
                                    vfields.iter().find(|(k, _)| k == field).map(|(_, v)| v)
                                else {
                                    return false;
                                };

                                // bind variable
                                self.define_var(field, v.clone());
                            }

                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) =
                                    vfields.iter().find(|(k, _)| k == field).map(|(_, v)| v)
                                else {
                                    return false;
                                };

                                if !self.host_bind_pattern(pat, v) {
                                    return false;
                                }
                            }

                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }
                EvalValue::EnumInstance { variant, args, .. } if variant == name => {
                    let mut index = 0;
                    for field in fields {
                        let Some(value) = args.get(index) else {
                            return false;
                        };
                        let ok = match field {
                            PatternField::Shorthand { field, .. } => {
                                self.define_var(field, value.clone());
                                true
                            }
                            PatternField::Assign { pat, .. } => self.host_bind_pattern(pat, value),
                            PatternField::Rest(_) => true,
                        };
                        if !ok {
                            return false;
                        }
                        if !matches!(field, PatternField::Rest(_)) {
                            index += 1;
                        }
                    }
                    index == args.len() || fields.iter().any(|f| matches!(f, PatternField::Rest(_)))
                }
                _ => false,
            },

            // Map pattern: map { x, y: pat, .. }
            Pattern::Map { fields, .. } => match value {
                EvalValue::Map(hm) => {
                    for pf in fields {
                        match pf {
                            // x  -> bind variable x = hm["x"]
                            PatternField::Shorthand { field, .. } => {
                                let Some(v) = hm.get(field) else {
                                    return false;
                                };
                                self.define_var(field, v.clone());
                            }

                            // x: pat -> recursively bind
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) = hm.get(field) else {
                                    return false;
                                };
                                if !self.host_bind_pattern(pat, v) {
                                    return false;
                                }
                            }

                            // .. -> allow extra keys
                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }

                _ => false,
            },

            // Enum pattern: Ok(x), Error(msg)
            Pattern::Variant { name, args, .. } => match value {
                EvalValue::EnumInstance {
                    variant,
                    args: v_args,
                    ..
                } => {
                    if variant != name {
                        return false;
                    }
                    if args.len() != v_args.len() {
                        return false;
                    }
                    for (p, v) in args.iter().zip(v_args.iter()) {
                        if !self.host_bind_pattern(p, v) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            },

            Pattern::Typed { pat, ty, .. } => {
                if !value.matches_type(ty) {
                    return false;
                }
                return self.host_bind_pattern(pat, value);
            }
        }
    }
}
