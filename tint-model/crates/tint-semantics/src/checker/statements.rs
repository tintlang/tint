impl SemanticChecker {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                pattern,
                ty,
                init,
                span: _,
            } => {
                // Init is checked against the scope BEFORE the new
                // binding exists, so `let x = x` correctly flags `x` as
                // unknown instead of seeing its own not-yet-bound name.
                if let Some(declared) = ty {
                    let expected = self.ast_type(Some(declared));
                    let found = self.infer_expr_with(init.expr(), Some(&expected));
                    self.require_compatible(&expected, &found, init.span());
                    self.bind_let(pattern, expected);
                } else {
                    let found = self.infer_expr(init.expr());
                    self.bind_let(pattern, found);
                }
                self.mark_pattern_mutable(pattern);
            }
            Stmt::Assign { lhs, rhs, span: _ } => {
                if let Expr::Ident(name, span) = lhs {
                    if self.scopes.lookup(name).is_some() && !self.scopes.is_mutable(name) {
                        self.error(*span, SemanticErrorKind::AssignToImmutable(name.clone()));
                    }
                }
                let lhs_ty = self.infer_expr(lhs);
                let rhs_ty = self.infer_expr_with(rhs, Some(&lhs_ty));
                self.require_compatible(&lhs_ty, &rhs_ty, rhs.span());
            }
            Stmt::CompoundAssign {
                name,
                op: _,
                expr,
                span,
            } => {
                if self.scopes.lookup(name).is_none() {
                    self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                } else if !self.scopes.is_mutable(name) {
                    self.error(*span, SemanticErrorKind::AssignToImmutable(name.clone()));
                }
                let lhs_ty = self.scopes.lookup(name).unwrap_or(Type::Unknown);
                let rhs_ty = self.infer_expr(expr);
                self.require_compatible(&lhs_ty, &rhs_ty, *span);
            }
            Stmt::Expr(e) => {
                self.visit_expr(e);
            }
            Stmt::If {
                cond,
                then,
                else_,
                span: _,
            } => {
                let cond_ty = self.infer_expr(cond);
                self.require_compatible(&Type::Bool, &cond_ty, cond.span());
                self.visit_block(then);
                if let Some(else_block) = else_ {
                    self.visit_block(else_block);
                }
            }
            Stmt::While {
                cond,
                body,
                span: _,
            } => {
                let cond_ty = self.infer_expr(cond);
                self.require_compatible(&Type::Bool, &cond_ty, cond.span());
                self.visit_block(body);
            }
            Stmt::Loop { body, span: _ } => {
                self.visit_block(body);
            }
            Stmt::For {
                var,
                start,
                end,
                body,
                span,
            } => {
                let start_ty = self.infer_expr(start);
                let end_ty = self.infer_expr(end);
                self.require_compatible(&Type::Number, &start_ty, start.span());
                self.require_compatible(&Type::Number, &end_ty, end.span());
                self.scopes.push();
                if !self.scopes.define(var, Type::Simple("i32".to_string())) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(var.clone()));
                }
                self.visit_block_in_current_scope(body);
                self.scopes.pop();
            }
            Stmt::ForIn { var, iter, body, span } => {
                let iter_ty = self.infer_expr(iter);
                let elem = match &iter_ty {
                    Type::Array(elem) => (**elem).clone(),
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            iter.span(),
                            SemanticErrorKind::TypeMismatch {
                                expected: "a list".into(),
                                found: format!("{other:?}"),
                            },
                        );
                        Type::Unknown
                    }
                };
                self.scopes.push();
                if !self.scopes.define(var, elem) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(var.clone()));
                }
                self.visit_block_in_current_scope(body);
                self.scopes.pop();
            }
            Stmt::Match {
                expr,
                arms,
                span: _,
            } => {
                let scrutinee = self.infer_expr(expr);
                self.check_match_arms(&scrutinee, arms);
            }
            Stmt::Return(e, span) => {
                let expected = self.current_return.clone();
                let found = self.infer_expr_with(e, expected.as_ref());
                if let Some(expected) = expected {
                    self.require_compatible(&expected, &found, *span);
                }
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn bind_let(&mut self, pattern: &Pattern, ty: Type) {
        self.rebind_allowed = true;
        self.bind_pattern_typed(pattern, ty);
        self.rebind_allowed = false;
    }

    fn check_match_arms(&mut self, scrutinee: &Type, arms: &[MatchArm]) {
        for arm in arms {
            self.scopes.push();
            self.bind_pattern_typed(&arm.pattern, scrutinee.clone());
            if let Some(guard) = &arm.guard {
                self.visit_expr(guard);
            }
            self.visit_expr(&arm.expr);
            self.scopes.pop();
        }
    }

    fn visit_struct_init_field(&mut self, field: &StructInitField) {
        match field {
            StructInitField::Assign { expr, .. } | StructInitField::Tint { expr, .. } => {
                self.visit_expr(expr)
            }
        }
    }

    // Expression
}
