use super::super::*;

impl TintVM {
    pub(super) fn host_capture_env(&mut self) -> std::rc::Rc<tint_evaluator::env::Env> {
        use tint_evaluator::env::Env;
        let mut env = Env::new();
        for (name, value) in self.scopes.values() {
            env.define(&name, Self::rt_to_eval(&value));
        }
        std::rc::Rc::new(env)
    }

    pub(super) fn host_match_pattern(
        &mut self,
        value: &EvalValue,
        pat: &tint_ast::Pattern,
    ) -> bool {
        use tint_ast::{Pattern, PatternField};

        match pat {
            // `_`
            Pattern::Wildcard(_) => true,

            // `mut x`
            Pattern::Mut { inner, .. } => self.host_match_pattern(value, inner),

            // `_ = expr`
            Pattern::Ident(_, _) => true,

            // literal numbers
            Pattern::Number(s, _) => match value {
                EvalValue::Number(n) => &n.to_string() == s,
                _ => false,
            },

            // literal string
            Pattern::String(s, _) => match value {
                EvalValue::String(v) => v == s,
                _ => false,
            },

            // group pattern: { a, b: pat }
            Pattern::Group { fields, .. } => match value {
                EvalValue::Map(map) => {
                    for pf in fields {
                        match pf {
                            // shorthand: { a }
                            PatternField::Shorthand { field, .. } => {
                                if !map.contains_key(field) {
                                    return false;
                                }
                            }

                            // assign: { a: pat }
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) = map.get(field) else {
                                    return false;
                                };

                                if !self.host_match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }

                _ => false,
            },

            // tuple patterns: (a, b, c)
            Pattern::Tuple(p_items, _) => match value {
                EvalValue::Tuple(v_items) => {
                    if p_items.len() != v_items.len() {
                        return false;
                    }

                    for (p, v) in p_items.iter().zip(v_items.iter()) {
                        if !self.host_match_pattern(v, p) {
                            return false;
                        }
                    }

                    true
                }
                _ => false,
            },

            // struct patterns: User { id, name }
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
                            // shorthand: name → just check exists
                            PatternField::Shorthand { field, .. } => {
                                if vfields.iter().all(|(k, _)| k != field) {
                                    return false;
                                }
                            }

                            // field: pattern
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) =
                                    vfields.iter().find(|(k, _)| k == field).map(|(_, v)| v)
                                else {
                                    return false;
                                };

                                if !self.host_match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            // `..` always ok
                            PatternField::Rest(_) => continue,
                        }
                    }

                    true
                }
                _ => false,
            },

            // map { x, y: pat, .. }
            Pattern::Map { fields, .. } => match value {
                EvalValue::Map(hm) => {
                    for pf in fields {
                        match pf {
                            // shorthand: x → must contain key "x"
                            PatternField::Shorthand { field, .. } => {
                                if !hm.contains_key(field) {
                                    return false;
                                }
                            }

                            // x: pat -> recursively match
                            PatternField::Assign { field, pat, .. } => {
                                let Some(v) = hm.get(field) else {
                                    return false;
                                };
                                if !self.host_match_pattern(v, pat) {
                                    return false;
                                }
                            }

                            // { .. } — allow other keys
                            PatternField::Rest(_) => continue,
                        }
                    }
                    true
                }
                _ => false,
            },

            // enums unsupported (as decided)
            Pattern::Variant { .. } => {
                panic!("Pattern matching on variants not supported");
            }

            // typed pattern: x: i32
            Pattern::Typed { pat, ty, .. } => {
                if !value.matches_type(ty) {
                    return false;
                }
                return self.host_match_pattern(value, pat);
            }
        }
    }
}
