impl SemanticChecker {
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(id, span) => {
                if self.scopes.lookup(id).is_none() {
                    self.error(*span, SemanticErrorKind::UnknownIdent(id.clone()));
                }
            }

            Expr::Number(_, _)
            | Expr::String(_, _)
            | Expr::Bool(_, _)
            | Expr::Unit(_)
            | Expr::SelfKw(_) => {}

            Expr::Call { target, args, span } => {
                // A bare-identifier callee is looked up as a FUNCTION
                // NAME (`known_fns`), not as a variable -- see the
                // `known_fns` doc comment on why these are different
                // namespaces. A variable holding a lambda is also
                // accepted, so `let f = |x| x + 1; f(2)` still works.
                if let Expr::Ident(name, _) = target.as_ref() {
                    if !self.known_fns.contains(name) && self.scopes.lookup(name).is_none() {
                        self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                    }
                } else {
                    self.visit_expr(target);
                }
                for arg in args {
                    self.visit_expr(arg);
                }
            }

            Expr::Array { items, span: _ } => {
                for item in items {
                    self.visit_expr(item);
                }
            }

            Expr::Unary {
                op: _,
                expr,
                span: _,
            } => {
                self.visit_expr(expr);
            }

            Expr::Binary {
                left,
                right,
                op: _,
                span: _,
            } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }

            Expr::Field {
                target,
                field: _,
                span: _,
            } => {
                self.visit_expr(target);
            }

            Expr::Index {
                target,
                index,
                span: _,
            } => {
                self.visit_expr(target);
                self.visit_expr(index);
            }

            Expr::InterpolatedString { parts, span: _ } => {
                for part in parts {
                    if let tint_ast::StringPart::Expr(e) = part {
                        self.visit_expr(e);
                    }
                }
            }

            Expr::Namespace { .. } => {
                // TODO: check namespace resolution -- needs a module
                // system to resolve against, which doesn't exist yet
                // (see docs/project-status.md: `use`/`mod`/`export`
                // parse but there's no multi-file loader).
            }

            Expr::Paren(inner, _) => self.visit_expr(inner),

            Expr::Match {
                scrutinee,
                arms,
                span: _,
            } => {
                self.visit_expr(scrutinee);
                self.check_match_arms(arms);
            }

            Expr::Lambda {
                params,
                body,
                span: _,
            } => {
                self.scopes.push();
                for p in params {
                    if !self.scopes.define(p, Type::Unit) {
                        // Span isn't tracked per-param here (`params` is
                        // just `Vec<String>`); attribute the error to the
                        // lambda body's own span instead of inventing one.
                        self.error(body.span(), SemanticErrorKind::DuplicateIdent(p.clone()));
                    }
                }
                self.visit_expr(body);
                self.scopes.pop();
            }

            Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => {
                for field in fields {
                    self.visit_struct_init_field(field);
                }
            }

            Expr::StructUpdate { base, updates, .. } => {
                self.visit_expr(base);
                for field in updates {
                    self.visit_struct_init_field(field);
                }
            }

            Expr::NamedArg { value, .. } => self.visit_expr(value),

            Expr::Block(block, _) => self.visit_block(block),

            Expr::Tuple { items, .. } => {
                for item in items {
                    self.visit_expr(item);
                }
            }

            Expr::TupleIndex { target, .. } => self.visit_expr(target),

            Expr::MapInit { entries, .. } => {
                for (_, e) in entries {
                    self.visit_expr(e);
                }
            }

            Expr::Borrow { target, block, .. } => {
                self.visit_expr(target);
                if let Some(block) = block {
                    self.visit_block(block);
                }
            }
        }
    }

    // Utility
    fn error(&mut self, span: Span, kind: SemanticErrorKind) {
        self.errors.push(SemanticError::new(kind, span));
    }
}
