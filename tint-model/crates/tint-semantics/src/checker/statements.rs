impl SemanticChecker {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                pattern,
                ty: _,
                init,
                span: _,
            } => {
                // Init is checked against the scope BEFORE the new
                // binding exists, so `let x = x` correctly flags `x` as
                // unknown instead of seeing its own not-yet-bound name.
                self.visit_expr(init.expr());
                self.bind_pattern(pattern);
            }
            Stmt::Assign { lhs, rhs, span: _ } => {
                if let Expr::Ident(name, span) = lhs {
                    if self.scopes.lookup(name).is_some() && !self.scopes.is_mutable(name) {
                        self.error(*span, SemanticErrorKind::AssignToImmutable(name.clone()));
                    }
                }
                self.visit_expr(lhs);
                self.visit_expr(rhs);
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
                self.visit_expr(expr);
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
                self.visit_expr(cond);
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
                self.visit_expr(cond);
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
                self.visit_expr(start);
                self.visit_expr(end);
                self.scopes.push();
                if !self.scopes.define(var, Type::Simple("i32".to_string())) {
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
                self.visit_expr(expr);
                self.check_match_arms(arms);
            }
            Stmt::Return(e, _) => {
                self.visit_expr(e);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn check_match_arms(&mut self, arms: &[MatchArm]) {
        for arm in arms {
            self.scopes.push();
            self.bind_pattern(&arm.pattern);
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
