use super::SsaCompiler;
use crate::ir::{Block, ValueId};
use std::collections::HashMap;
use tint_ast::{Expr, Pattern, PatternField, Stmt};

impl SsaCompiler {
    pub(super) fn lower_assignment_lhs(
        &mut self,
        lhs: &Expr,
        value: ValueId,
        block: &mut Block,
        locals: &mut HashMap<String, ValueId>,
    ) {
        match lhs {
            // x = v
            Expr::Ident(name, _) => {
                self.builder.emit_store(block, name.clone(), value);
                locals.insert(name.clone(), value);
            }

            // obj.field = v
            Expr::Field { target, field, .. } => {
                let base = self.lower_expr(target, block, locals);
                self.builder
                    .emit_field_store(block, base, field.clone(), value);
            }

            // arr[i] = v
            Expr::Index { target, index, .. } => {
                let arr = self.lower_expr(target, block, locals);
                let idx = self.lower_expr(index, block, locals);
                self.builder.emit_index_store(block, arr, idx, value);
            }

            // Invalid assignment LHS
            _ => {
                panic!("Invalid assignment LHS: {:?}", lhs);
            }
        }
    }
    // LOWERING STATEMENT
    pub(super) fn lower_stmt(
        &mut self,
        s: &Stmt,
        block: &mut Block,
        locals: &mut std::collections::HashMap<String, ValueId>,
    ) {
        match s {
            // NEW: let pattern = expr
            Stmt::Let { pattern, init, .. } => {
                let expr = match init {
                    tint_ast::LetInit::Assign(e) => e,
                    tint_ast::LetInit::Tint(e) => e,
                };

                let v = self.lower_expr(expr, block, locals);
                self.bind_pattern(pattern, v, locals, block);
            }

            Stmt::Assign { lhs, rhs, .. } => {
                let rv = self.lower_expr(rhs, block, locals);
                self.lower_assignment_lhs(lhs, rv, block, locals);
            }

            Stmt::Return(expr, _) => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_return(block, v);
            }

            Stmt::Expr(expr) => {
                self.lower_expr(expr, block, locals);
            }

            _ => {}
        }
    }

    // PATTERN BINDING in SS
    pub(super) fn bind_pattern(
        &mut self,
        pat: &Pattern,
        value: ValueId,
        locals: &mut HashMap<String, ValueId>,
        block: &mut Block,
    ) {
        match pat {
            // x
            Pattern::Ident(name, _) => {
                locals.insert(name.clone(), value);
            }

            // _
            Pattern::Wildcard(_) => {}

            Pattern::Mut { inner, .. } => {
                self.bind_pattern(inner, value, locals, block);
            }

            // NUMBER | STRING
            Pattern::Number(_, _) => {}
            Pattern::String(_, _) => {}

            // (a, b, c)
            Pattern::Tuple(items, _) => {
                for (i, subpat) in items.iter().enumerate() {
                    let elem = self.builder.emit_tuple_extract(block, value, i);
                    self.bind_pattern(subpat, elem, locals, block);
                }
            }

            // User { id, name }
            Pattern::Struct { fields, .. } => {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            let fld = self.builder.emit_field_access(block, value, field.clone());
                            locals.insert(field.clone(), fld);
                        }

                        PatternField::Assign { field, pat, .. } => {
                            let fld = self.builder.emit_field_access(block, value, field.clone());
                            self.bind_pattern(pat, fld, locals, block);
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            }

            // { a, b: pat } — group pattern (struct-like, but without type)
            Pattern::Group { fields, .. } => {
                for f in fields {
                    match f {
                        // shorthand: { a }
                        PatternField::Shorthand { field, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            locals.insert(field.clone(), elem);
                        }

                        // assign: { a: pat }
                        PatternField::Assign { field, pat, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            self.bind_pattern(pat, elem, locals, block);
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            }

            // map { x, y: pat, .. }
            Pattern::Map { fields, .. } => {
                for f in fields {
                    match f {
                        // shorthand: map { x }
                        PatternField::Shorthand { field, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            locals.insert(field.clone(), elem);
                        }

                        // field: pat
                        PatternField::Assign { field, pat, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            self.bind_pattern(pat, elem, locals, block);
                        }

                        // ..
                        PatternField::Rest(_) => {}
                    }
                }
            }

            // Some(x), Ok(v) -- still a stub
            Pattern::Variant { .. } => {}

            // typed pattern:  pat: Type
            Pattern::Typed { pat, .. } => {
                // just recurse into the inner pattern
                self.bind_pattern(pat, value, locals, block);
            }
        }
    }
}
