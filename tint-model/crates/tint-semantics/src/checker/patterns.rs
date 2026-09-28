impl SemanticChecker {
    fn check_pattern_type(&mut self, pattern: &Pattern, expected: &Type) {
        match pattern {
            Pattern::Number(_, span) => self.require_compatible(&Type::Number, expected, *span),
            Pattern::String(_, span) => self.require_compatible(&Type::String, expected, *span),
            Pattern::Typed { pat, ty, span } => {
                let declared = self.ast_type(Some(ty));
                self.require_compatible(&declared, expected, *span);
                self.check_pattern_type(pat, &declared);
            }
            Pattern::Tuple(items, span) => {
                if let Type::Tuple(types) = expected {
                    if items.len() != types.len() {
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: "tuple of matching arity".into(),
                                found: "tuple".into(),
                            },
                        );
                    }
                    for (item, ty) in items.iter().zip(types) {
                        self.check_pattern_type(item, ty);
                    }
                }
            }
            Pattern::Struct { name, fields, span } => {
                let actual = match expected {
                    Type::Struct(actual) => actual,
                    Type::Unknown => return,
                    _ => {
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: name.clone(),
                                found: type_name(expected),
                            },
                        );
                        return;
                    }
                };
                if actual != name {
                    self.error(
                        *span,
                        SemanticErrorKind::TypeMismatch {
                            expected: name.clone(),
                            found: actual.clone(),
                        },
                    );
                    return;
                }
                for field in fields {
                    if let PatternField::Assign {
                        pat,
                        field: field_name,
                        ..
                    } = field
                    {
                        let ty = self
                            .struct_fields
                            .get(name)
                            .and_then(|fields| fields.get(field_name))
                            .cloned()
                            .unwrap_or(Type::Unknown);
                        self.check_pattern_type(pat, &ty);
                    }
                }
            }
            Pattern::Variant { name, args, span } => {
                if let Type::Enum(enum_name) = expected {
                    if let Some(types) =
                        self.enum_variants
                            .iter()
                            .find_map(|((decl, variant), types)| {
                                (decl == enum_name && variant == name).then_some(types.clone())
                            })
                    {
                        if args.len() != types.len() {
                            self.error(
                                *span,
                                SemanticErrorKind::TypeMismatch {
                                    expected: format!("{} arguments", types.len()),
                                    found: format!("{} arguments", args.len()),
                                },
                            );
                        }
                        for (arg, ty) in args.iter().zip(types) {
                            self.check_pattern_type(arg, &ty);
                        }
                    } else {
                        self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                    }
                }
            }
            Pattern::Map { fields, .. } | Pattern::Group { fields, .. } => {
                let item = match expected {
                    Type::Map(item) => (**item).clone(),
                    Type::Unknown => Type::Unknown,
                    _ => Type::Unknown,
                };
                for field in fields {
                    if let PatternField::Assign { pat, .. } = field {
                        self.check_pattern_type(pat, &item);
                    }
                }
            }
            Pattern::Mut { inner, .. } => self.check_pattern_type(inner, expected),
            Pattern::Ident(_, _) | Pattern::Wildcard(_) => {}
        }
    }

    fn check_match_exhaustiveness(&mut self, scrutinee: &Type, arms: &[MatchArm]) {
        if arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard(_)) && arm.guard.is_none())
        {
            return;
        }
        match scrutinee {
            Type::Bool => {}
            Type::Enum(name) => {
                let variants: HashSet<String> = self
                    .enum_variants
                    .keys()
                    .filter_map(|(decl, variant)| (decl == name).then_some(variant.clone()))
                    .collect();
                let covered: HashSet<String> = arms
                    .iter()
                    .filter(|arm| arm.guard.is_none())
                    .filter_map(|arm| match &arm.pattern {
                        Pattern::Variant { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .collect();
                if !variants.is_empty() && !variants.is_subset(&covered) {
                    self.error(
                        arms.first().map(|arm| arm.span).unwrap_or(Span::dummy()),
                        SemanticErrorKind::MissingMatchArms,
                    );
                }
            }
            _ => {}
        }
    }

    fn bind_pattern_typed(&mut self, pattern: &Pattern, ty: Type) {
        match pattern {
            Pattern::Ident(name, span) => {
                if !self.scopes.define_with_mutability(name, ty, false) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(name.clone()));
                }
            }
            Pattern::Typed {
                pat, ty: declared, ..
            } => {
                let declared = self.ast_type(Some(declared));
                self.require_compatible(&declared, &ty, pattern.span());
                self.bind_pattern_typed(pat, declared);
            }
            Pattern::Mut { inner, .. } => self.bind_pattern_typed(inner, ty),
            Pattern::Tuple(items, _) => {
                let members = match ty {
                    Type::Tuple(items) => items,
                    _ => Vec::new(),
                };
                for (index, item) in items.iter().enumerate() {
                    self.bind_pattern_typed(
                        item,
                        members.get(index).cloned().unwrap_or(Type::Unknown),
                    );
                }
            }
            Pattern::Struct { fields, .. }
            | Pattern::Map { fields, .. }
            | Pattern::Group { fields, .. } => {
                for field in fields {
                    self.bind_pattern_field_typed(field, Type::Unknown);
                }
            }
            Pattern::Variant { args, .. } => {
                for arg in args {
                    self.bind_pattern_typed(arg, Type::Unknown);
                }
            }
            Pattern::Number(_, _) | Pattern::String(_, _) | Pattern::Wildcard(_) => {}
        }
    }

    fn mark_pattern_mutable(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Mut { inner, .. } => self.mark_pattern_mutable_inner(inner),
            Pattern::Tuple(items, _) => {
                for item in items {
                    self.mark_pattern_mutable(item);
                }
            }
            Pattern::Typed { pat, .. } => self.mark_pattern_mutable(pat),
            _ => {}
        }
    }

    fn mark_pattern_mutable_inner(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident(name, _) => self.scopes.mark_mutable(name),
            Pattern::Tuple(items, _) => {
                for item in items {
                    self.mark_pattern_mutable_inner(item);
                }
            }
            Pattern::Typed { pat, .. } | Pattern::Mut { inner: pat, .. } => {
                self.mark_pattern_mutable_inner(pat)
            }
            _ => {}
        }
    }

    fn bind_pattern_field_typed(&mut self, field: &PatternField, ty: Type) {
        match field {
            PatternField::Shorthand { field, span } => {
                if !self.scopes.define(field, ty) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(field.clone()));
                }
            }
            PatternField::Assign { pat, .. } => self.bind_pattern_typed(pat, ty),
            PatternField::Rest(_) => {}
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern) {
        self.bind_pattern_with_mutability(pattern, false);
    }

    fn bind_pattern_with_mutability(&mut self, pattern: &Pattern, is_mutable: bool) {
        match pattern {
            Pattern::Ident(name, span) => {
                if !self
                    .scopes
                    .define_with_mutability(name, Type::Unit, is_mutable)
                {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(name.clone()));
                }
            }
            Pattern::Tuple(items, _) => {
                for p in items {
                    self.bind_pattern_with_mutability(p, is_mutable);
                }
            }
            Pattern::Struct { fields, .. }
            | Pattern::Map { fields, .. }
            | Pattern::Group { fields, .. } => {
                for field in fields {
                    self.bind_pattern_field_with_mutability(field, is_mutable);
                }
            }
            Pattern::Variant { args, .. } => {
                for p in args {
                    self.bind_pattern_with_mutability(p, is_mutable);
                }
            }
            Pattern::Typed { pat, .. } => self.bind_pattern_with_mutability(pat, is_mutable),
            Pattern::Mut { inner, .. } => self.bind_pattern_with_mutability(inner, true),
            Pattern::Number(_, _) | Pattern::String(_, _) | Pattern::Wildcard(_) => {}
        }
    }

    fn bind_pattern_field_with_mutability(&mut self, field: &PatternField, is_mutable: bool) {
        match field {
            PatternField::Shorthand { field, span } => {
                if !self
                    .scopes
                    .define_with_mutability(field, Type::Unit, is_mutable)
                {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(field.clone()));
                }
            }
            PatternField::Assign { pat, .. } => self.bind_pattern_with_mutability(pat, is_mutable),
            PatternField::Rest(_) => {}
        }
    }

    // Block (logic)
    fn visit_block(&mut self, block: &Block) {
        self.scopes.push();
        self.visit_block_in_current_scope(block);
        self.scopes.pop();
    }

    // Like `visit_block`, but checks the statements in whatever scope is
    // already on top -- used when a caller (a function body, a `for`
    // loop already holding its loop variable, ...) needs its own
    // bindings to be visible to the block's statements rather than
    // shadowed by a second, immediately-pushed frame.
    fn visit_block_in_current_scope(&mut self, block: &Block) -> Type {
        self.ctx.mode = Mode::Logic;
        let mut last = Type::Unit;
        for stmt in &block.stmts {
            if let Stmt::Expr(expr) = stmt {
                last = self.infer_expr(expr);
                continue;
            }
            self.visit_stmt(stmt);
        }
        last
    }
}
