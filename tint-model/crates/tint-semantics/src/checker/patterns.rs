/// What a struct-like pattern (`Point { x, y }`, `Circle { r }`, `Some(x)`)
/// destructures.
enum PatShape {
    /// Field names (`None` for positional variant fields) and their types.
    Fields(Vec<(Option<String>, Type)>),
    /// The scrutinee type is not known; bind everything as unknown.
    Unknown,
    /// Already reported.
    Bad,
}

impl SemanticChecker {
    fn pattern_shape(&mut self, name: &str, expected: &Type, span: Span) -> PatShape {
        let mut current = self.shallow(expected);
        if let Type::Var(_) = current {
            // A pattern that names a struct or a variant tells what is matched.
            let guess = if self.struct_fields.contains_key(name) {
                let params = self.generic_params_by_type.get(name).cloned().unwrap_or_default();
                if params.is_empty() {
                    Some(Type::Struct(name.to_string()))
                } else {
                    let vars = params.iter().map(|p| self.fresh_hole(span, format!("type parameter `{p}` of `{name}`"))).collect();
                    Some(Type::Generic(name.to_string(), vars))
                }
            } else {
                self.enum_of_variant(name)
                    .map(|owner| self.instantiate_variant(&owner, name, span).0)
            };
            match guess {
                Some(ty) => {
                    self.unify(&current, &ty);
                    current = self.shallow(&ty);
                }
                None => {
                    self.error(span, SemanticErrorKind::UnknownIdent(name.to_string()));
                    return PatShape::Bad;
                }
            }
        }
        match current {
            Type::Unknown => PatShape::Unknown,
            Type::Struct(actual) if actual == name => PatShape::Fields(
                self.struct_fields
                    .get(name)
                    .map(|f| f.iter().map(|(k, v)| (Some(k.clone()), v.clone())).collect())
                    .unwrap_or_default(),
            ),
            Type::Generic(actual, args) if actual == name && self.struct_fields.contains_key(name) => {
                let params = self.generic_params_by_type.get(name).cloned().unwrap_or_default();
                let substitutions: HashMap<String, Type> = params.into_iter().zip(args).collect();
                PatShape::Fields(
                    self.struct_fields[name]
                        .iter()
                        .map(|(k, v)| (Some(k.clone()), substitute_type(v, &substitutions)))
                        .collect(),
                )
            }
            Type::Enum(owner) => self.variant_shape(&owner, &[], name, span),
            Type::Generic(owner, args) if self.enum_names.contains(&owner) => {
                self.variant_shape(&owner, &args, name, span)
            }
            found => {
                let found = self.resolve(&found);
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: name.to_string(),
                        found: type_name(&found),
                    },
                );
                PatShape::Bad
            }
        }
    }

    fn variant_shape(&mut self, owner: &str, args: &[Type], name: &str, span: Span) -> PatShape {
        let key = (owner.to_string(), name.to_string());
        let Some(payload) = self.enum_variants.get(&key).cloned() else {
            self.error(span, SemanticErrorKind::UnknownIdent(name.to_string()));
            return PatShape::Bad;
        };
        let params = self.generic_params_by_type.get(owner).cloned().unwrap_or_default();
        let substitutions: HashMap<String, Type> = params.into_iter().zip(args.iter().cloned()).collect();
        let names = self.variant_fields.get(&key).cloned().unwrap_or_default();
        PatShape::Fields(
            payload
                .iter()
                .enumerate()
                .map(|(i, ty)| (names.get(i).cloned().flatten(), substitute_type(ty, &substitutions)))
                .collect(),
        )
    }

    fn bind_name(&mut self, name: &str, span: Span, ty: Type) {
        self.binding_types
            .insert((span.start.offset, span.end.offset), ty.clone());
        if self.rebind_allowed {
            self.scopes.redefine(name, ty);
        } else if !self.scopes.define_with_mutability(name, ty, false) {
            self.error(span, SemanticErrorKind::DuplicateIdent(name.to_string()));
        }
    }

    /// Checks `pattern` against the type of what it matches and binds the
    /// names it introduces, each with its own type.
    fn bind_pattern_typed(&mut self, pattern: &Pattern, expected: Type) {
        match pattern {
            Pattern::Ident(name, span) => self.bind_name(name, *span, expected),
            Pattern::Number(_, span) => self.require_compatible(&Type::Number, &expected, *span),
            Pattern::String(_, span) => self.require_compatible(&Type::String, &expected, *span),
            Pattern::Wildcard(_) => {}
            Pattern::Typed { pat, ty, span } => {
                let declared = self.ast_type(Some(ty));
                self.require_compatible(&declared, &expected, *span);
                self.bind_pattern_typed(pat, declared);
            }
            Pattern::Mut { inner, .. } => self.bind_pattern_typed(inner, expected),
            Pattern::Tuple(items, span) => {
                let members = match self.shallow(&expected) {
                    Type::Tuple(types) => {
                        if types.len() != items.len() {
                            self.error(
                                *span,
                                SemanticErrorKind::TypeMismatch {
                                    expected: "tuple of matching arity".into(),
                                    found: "tuple".into(),
                                },
                            );
                        }
                        types
                    }
                    Type::Var(_) => {
                        let vars: Vec<Type> = items
                            .iter()
                            .map(|_| self.fresh(*span, "a tuple element"))
                            .collect();
                        self.unify(&expected, &Type::Tuple(vars.clone()));
                        vars
                    }
                    Type::Unknown => Vec::new(),
                    found => {
                        let found = self.resolve(&found);
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: "tuple".into(),
                                found: type_name(&found),
                            },
                        );
                        Vec::new()
                    }
                };
                for (index, item) in items.iter().enumerate() {
                    self.bind_pattern_typed(item, members.get(index).cloned().unwrap_or(Type::Unknown));
                }
            }
            Pattern::Struct { name, fields, span } => {
                let shape = self.pattern_shape(name, &expected, *span);
                for field in fields {
                    match field {
                        PatternField::Shorthand { field: field_name, span } => {
                            let ty = self.shape_field(&shape, name, field_name, *span);
                            self.bind_name(field_name, *span, ty);
                        }
                        PatternField::Assign { field: field_name, pat, span } => {
                            let ty = self.shape_field(&shape, name, field_name, *span);
                            self.bind_pattern_typed(pat, ty);
                        }
                        PatternField::Rest(_) => {}
                    }
                }
            }
            Pattern::Variant { name, args, span } => {
                let shape = self.pattern_shape(name, &expected, *span);
                if let PatShape::Fields(fields) = &shape {
                    if fields.len() != args.len() {
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: format!("{} arguments", fields.len()),
                                found: format!("{} arguments", args.len()),
                            },
                        );
                    }
                }
                for (index, arg) in args.iter().enumerate() {
                    let ty = match &shape {
                        PatShape::Fields(fields) => fields.get(index).map(|f| f.1.clone()),
                        _ => None,
                    };
                    self.bind_pattern_typed(arg, ty.unwrap_or(Type::Unknown));
                }
            }
            Pattern::Map { fields, span } | Pattern::Group { fields, span } => {
                let item = match self.shallow(&expected) {
                    Type::Map(item) => *item,
                    Type::Var(_) => {
                        let item = self.fresh_hole(*span, "a map value");
                        self.unify(&expected, &Type::Map(Box::new(item.clone())));
                        item
                    }
                    _ => Type::Unknown,
                };
                for field in fields {
                    match field {
                        PatternField::Shorthand { field, span } => {
                            self.bind_name(field, *span, item.clone())
                        }
                        PatternField::Assign { pat, .. } => {
                            self.bind_pattern_typed(pat, item.clone())
                        }
                        PatternField::Rest(_) => {}
                    }
                }
            }
        }
    }

    fn shape_field(&mut self, shape: &PatShape, owner: &str, field: &str, span: Span) -> Type {
        match shape {
            PatShape::Fields(fields) => {
                match fields.iter().find(|(n, _)| n.as_deref() == Some(field)) {
                    Some((_, ty)) => ty.clone(),
                    None => {
                        self.error(
                            span,
                            SemanticErrorKind::UnknownIdent(format!("{owner}.{field}")),
                        );
                        Type::Unknown
                    }
                }
            }
            PatShape::Unknown | PatShape::Bad => Type::Unknown,
        }
    }

    fn check_match_exhaustiveness(&mut self, scrutinee: &Type, arms: &[MatchArm]) {
        if arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard(_)) && arm.guard.is_none())
        {
            return;
        }
        match self.shallow(scrutinee) {
            Type::Enum(name) | Type::Generic(name, _) => {
                let variants: HashSet<String> = self
                    .enum_variants
                    .keys()
                    .filter_map(|(decl, variant)| (decl == &name).then_some(variant.clone()))
                    .collect();
                let covered: HashSet<String> = arms
                    .iter()
                    .filter(|arm| arm.guard.is_none())
                    .filter_map(|arm| match &arm.pattern {
                        Pattern::Variant { name, .. } => Some(name.clone()),
                        Pattern::Struct { name, .. } => Some(name.clone()),
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

    // Block (logic)
    fn visit_block(&mut self, block: &Block) {
        self.scopes.push();
        self.visit_block_in_current_scope(block);
        self.scopes.pop();
    }

    /// Like `visit_block`, but checks the statements in whatever scope is
    /// already on top -- used when a caller (a function body, a `for`
    /// loop already holding its loop variable, ...) needs its own
    /// bindings to be visible to the block's statements rather than
    /// shadowed by a second, immediately-pushed frame.
    ///
    /// Returns the block's value: its last statement when that is an
    /// expression, `unit` otherwise, and an open hole after `return` /
    /// `break` / `continue` (nothing follows, so nothing constrains it).
    fn visit_block_in_current_scope(&mut self, block: &Block) -> Type {
        self.ctx.mode = Mode::Logic;
        let mut value = Type::Unit;
        let count = block.stmts.len();
        for (index, stmt) in block.stmts.iter().enumerate() {
            let last = index + 1 == count;
            match stmt {
                Stmt::Expr(expr) => {
                    let ty = self.infer_expr(expr);
                    if last {
                        value = ty;
                    }
                }
                Stmt::Return(..) | Stmt::Break(_) | Stmt::Continue(_) => {
                    self.visit_stmt(stmt);
                    if last {
                        value = self.fresh_hole(block.span, "a branch that never completes");
                    }
                }
                // A `loop` nothing breaks out of only ends through `return`.
                Stmt::Loop { body, .. } if !block_breaks(body) => {
                    self.visit_stmt(stmt);
                    if last {
                        value = self.fresh_hole(block.span, "a loop that never ends");
                    }
                }
                _ => self.visit_stmt(stmt),
            }
        }
        value
    }
}

/// Does a `break` in this block leave the loop that owns the block? Breaks
/// inside nested loops leave those loops instead.
fn block_breaks(block: &Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Break(_) => true,
        Stmt::If { then, else_, .. } => {
            block_breaks(then) || else_.as_ref().is_some_and(block_breaks)
        }
        Stmt::Expr(Expr::If { then, else_, .. }) => block_breaks(then) || block_breaks(else_),
        Stmt::Expr(Expr::Block(inner, _)) => block_breaks(inner),
        _ => false,
    })
}
