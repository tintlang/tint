impl SemanticChecker {
    fn visit_expr(&mut self, expr: &Expr) {
        let _ = self.infer_expr(expr);
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Number(_, _) => Type::Number,
            Expr::String(_, _) => Type::String,
            Expr::Bool(_, _) => Type::Bool,
            Expr::Unit(_) => Type::Unit,
            Expr::Ident(id, span) => self.scopes.lookup(id).unwrap_or_else(|| {
                self.error(*span, SemanticErrorKind::UnknownIdent(id.clone()));
                Type::Unknown
            }),
            Expr::SelfKw(span) => self.scopes.lookup("self").unwrap_or_else(|| {
                self.error(*span, SemanticErrorKind::UnknownIdent("self".into()));
                Type::Unknown
            }),
            Expr::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let tint_ast::StringPart::Expr(e) = part {
                        self.infer_expr(e);
                    }
                }
                Type::String
            }
            Expr::Call { target, args, span } => self.infer_call(target, args, *span),
            Expr::Field {
                target,
                field,
                span,
            } => match self.infer_expr(target) {
                Type::Struct(name) => self
                    .struct_fields
                    .get(&name)
                    .and_then(|f| f.get(field))
                    .cloned()
                    .unwrap_or_else(|| {
                        self.error(
                            *span,
                            SemanticErrorKind::UnknownIdent(format!("{}.{}", name, field)),
                        );
                        Type::Unknown
                    }),
                Type::Unknown => Type::Unknown,
                found => {
                    self.error(
                        *span,
                        SemanticErrorKind::TypeMismatch {
                            expected: "struct".into(),
                            found: type_name(&found),
                        },
                    );
                    Type::Unknown
                }
            },
            Expr::Array { items, .. } => {
                let mut item_ty = Type::Unknown;
                for item in items {
                    let found = self.infer_expr(item);
                    if matches!(item_ty, Type::Unknown) {
                        item_ty = found;
                    } else {
                        self.require_compatible(&item_ty, &found, item.span());
                    }
                }
                Type::Array(Box::new(item_ty))
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let target_ty = self.infer_expr(target);
                let index_ty = self.infer_expr(index);
                self.require_compatible(&Type::Number, &index_ty, index.span());
                match target_ty {
                    Type::Array(item) | Type::Map(item) => *item,
                    Type::Unknown => Type::Unknown,
                    found => {
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: "array or map".into(),
                                found: type_name(&found),
                            },
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::Unary { op, expr, span } => {
                let ty = self.infer_expr(expr);
                if op == "!" {
                    self.require_compatible(&Type::Bool, &ty, *span);
                    Type::Bool
                } else {
                    self.require_compatible(&Type::Number, &ty, *span);
                    Type::Number
                }
            }
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let l = self.infer_expr(left);
                let r = self.infer_expr(right);
                match op.as_str() {
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        self.require_compatible(&l, &r, *span);
                        Type::Bool
                    }
                    "&&" | "||" => {
                        self.require_compatible(&Type::Bool, &l, left.span());
                        self.require_compatible(&Type::Bool, &r, right.span());
                        Type::Bool
                    }
                    "+" if matches!(l, Type::String) || matches!(r, Type::String) => {
                        self.require_compatible(&Type::String, &l, left.span());
                        self.require_compatible(&Type::String, &r, right.span());
                        Type::String
                    }
                    _ => {
                        self.require_compatible(&Type::Number, &l, left.span());
                        self.require_compatible(&Type::Number, &r, right.span());
                        Type::Number
                    }
                }
            }
            Expr::Paren(inner, _) => self.infer_expr(inner),
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let scrutinee_ty = self.infer_expr(scrutinee);
                let mut result = Type::Unknown;
                for arm in arms {
                    self.scopes.push();
                    self.check_pattern_type(&arm.pattern, &scrutinee_ty);
                    self.bind_pattern_typed(&arm.pattern, scrutinee_ty.clone());
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.infer_expr(guard);
                        self.require_compatible(&Type::Bool, &guard_ty, guard.span());
                    }
                    let arm_ty = self.infer_expr(&arm.expr);
                    if matches!(result, Type::Unknown) {
                        result = arm_ty;
                    } else {
                        self.require_compatible(&result, &arm_ty, arm.expr.span());
                    }
                    self.scopes.pop();
                }
                self.check_match_exhaustiveness(&scrutinee_ty, arms);
                result
            }
            Expr::Lambda { params, body, .. } => {
                self.scopes.push();
                for param in params {
                    self.scopes.define(param, Type::Unknown);
                }
                let ret = self.infer_expr(body);
                self.scopes.pop();
                Type::Fn(Box::new(ret), vec![Type::Unknown; params.len()])
            }
            Expr::StructInit { name, fields, span } => {
                if let Some(expected) = self.struct_fields.get(name).cloned() {
                    let mut seen = HashSet::new();
                    for field in fields {
                        let (field_name, value, field_span) = init_field_parts(field);
                        let found = self.infer_expr(value);
                        if let Some(field_ty) = expected.get(field_name) {
                            self.require_compatible(field_ty, &found, *field_span);
                            seen.insert(field_name.to_string());
                        } else {
                            self.error(
                                *field_span,
                                SemanticErrorKind::UnknownIdent(format!("{}.{}", name, field_name)),
                            );
                        }
                    }
                    for field_name in expected.keys() {
                        if !seen.contains(field_name)
                            && !matches!(expected.get(field_name), Some(Type::Unknown))
                        {
                            self.error(*span, SemanticErrorKind::Unsupported);
                        }
                    }
                } else if !self.type_names.contains(name) && name != "Vec2" {
                    self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                }
                Type::Struct(name.clone())
            }
            Expr::StructUpdate { base, updates, .. } => {
                let ty = self.infer_expr(base);
                for field in updates {
                    self.visit_struct_init_field(field);
                }
                ty
            }
            Expr::NamedArg { value, .. } => self.infer_expr(value),
            Expr::Block(block, _) => self.infer_block_type(block),
            Expr::Tuple { items, .. } => {
                Type::Tuple(items.iter().map(|item| self.infer_expr(item)).collect())
            }
            Expr::TupleIndex {
                target,
                index,
                span,
            } => match self.infer_expr(target) {
                Type::Tuple(items) => items.get(*index).cloned().unwrap_or_else(|| {
                    self.error(*span, SemanticErrorKind::Unsupported);
                    Type::Unknown
                }),
                Type::Unknown => Type::Unknown,
                found => {
                    self.error(
                        *span,
                        SemanticErrorKind::TypeMismatch {
                            expected: "tuple".into(),
                            found: type_name(&found),
                        },
                    );
                    Type::Unknown
                }
            },
            Expr::VariantInit {
                enum_name,
                variant,
                fields,
                span,
            } => {
                if let Some(expected) = self
                    .enum_variants
                    .get(&(enum_name.clone(), variant.clone()))
                    .cloned()
                {
                    if expected.len() != fields.len() {
                        self.error(*span, SemanticErrorKind::Unsupported);
                    }
                    for (index, field) in fields.iter().enumerate() {
                        let (_, value, field_span) = init_field_parts(field);
                        let found = self.infer_expr(value);
                        if let Some(expected_ty) = expected.get(index) {
                            self.require_compatible(expected_ty, &found, *field_span);
                        }
                    }
                } else {
                    self.error(
                        *span,
                        SemanticErrorKind::UnknownIdent(format!("{}::{}", enum_name, variant)),
                    );
                    for field in fields {
                        self.visit_struct_init_field(field);
                    }
                }
                Type::Enum(enum_name.clone())
            }
            Expr::MapInit { entries, .. } => {
                let mut item = Type::Unknown;
                for (_, value) in entries {
                    let found = self.infer_expr(value);
                    if matches!(item, Type::Unknown) {
                        item = found;
                    } else {
                        self.require_compatible(&item, &found, value.span());
                    }
                }
                Type::Map(Box::new(item))
            }
            Expr::Borrow { target, block, .. } => {
                let ty = self.infer_expr(target);
                if let Some(block) = block {
                    self.visit_block(block);
                }
                ty
            }
            Expr::Namespace { base, .. } => {
                self.infer_expr(base);
                Type::Unknown
            }
        }
    }

    fn infer_call(&mut self, target: &Expr, args: &[Expr], span: Span) -> Type {
        let arg_types: Vec<Type> = args.iter().map(|arg| self.infer_expr(arg)).collect();
        if let Expr::Namespace { base, item, .. } = target {
            if let Expr::Ident(enum_name, _) = base.as_ref() {
                if let Some(params) = self
                    .enum_variants
                    .get(&(enum_name.clone(), item.clone()))
                    .cloned()
                {
                    self.check_args(&params, &arg_types, span);
                    return Type::Enum(enum_name.clone());
                }
            }
        }
        if let Expr::Field {
            target: receiver,
            field: method,
            ..
        } = target
        {
            let receiver_ty = self.infer_expr(receiver);
            if let Type::Struct(name) = receiver_ty {
                if let Some((params, ret)) = self.methods.get(&(name, method.clone())).cloned() {
                    self.check_args(&params, &arg_types, span);
                    return ret;
                }
            }
        }
        if let Expr::Ident(name, _) = target {
            if let Some((params, ret)) = self.fn_types.get(name).cloned() {
                self.check_args(&params, &arg_types, span);
                return ret;
            }
            if let Some(Type::Fn(ret, params)) = self.scopes.lookup(name) {
                self.check_args(&params, &arg_types, span);
                return *ret;
            }
            match name.as_str() {
                "vec2" => {
                    self.check_args(&[Type::Number, Type::Number], &arg_types, span);
                    return Type::Struct("Vec2".into());
                }
                "clamp" | "min" | "max" | "abs" | "sign" | "sqrt" => return Type::Number,
                _ => {
                    if !self.known_fns.contains(name) {
                        self.error(span, SemanticErrorKind::UnknownIdent(name.clone()));
                    }
                    return Type::Unknown;
                }
            }
        }
        match self.infer_expr(target) {
            Type::Fn(ret, params) => {
                self.check_args(&params, &arg_types, span);
                *ret
            }
            Type::Unknown => Type::Unknown,
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "function".into(),
                        found: type_name(&found),
                    },
                );
                Type::Unknown
            }
        }
    }

    fn check_args(&mut self, params: &[Type], args: &[Type], span: Span) {
        if params.len() != args.len() {
            self.error(span, SemanticErrorKind::Unsupported);
        }
        for (expected, found) in params.iter().zip(args) {
            self.require_compatible(expected, found, span);
        }
    }
    fn require_compatible(&mut self, expected: &Type, found: &Type, span: Span) {
        if !compatible(expected, found) {
            self.error(
                span,
                SemanticErrorKind::TypeMismatch {
                    expected: type_name(expected),
                    found: type_name(found),
                },
            );
        }
    }
    fn error(&mut self, span: Span, kind: SemanticErrorKind) {
        self.errors.push(SemanticError::new(kind, span));
    }

    fn infer_block_type(&mut self, block: &Block) -> Type {
        self.scopes.push();
        let ty = self.visit_block_in_current_scope(block);
        self.scopes.pop();
        ty
    }
}

fn compatible(expected: &Type, found: &Type) -> bool {
    if matches!(expected, Type::Unknown) || matches!(found, Type::Unknown) {
        return true;
    }
    if let (Type::Simple(a), Type::Simple(b)) = (expected, found) {
        return a == b;
    }
    if let Type::Simple(name) = expected {
        return matches!(
            (name.as_str(), found),
            ("f32" | "f64" | "i32" | "i64" | "u32" | "u64", Type::Number)
        );
    }
    if let Type::Simple(name) = found {
        return matches!(
            (name.as_str(), expected),
            ("f32" | "f64" | "i32" | "i64" | "u32" | "u64", Type::Number)
        );
    }
    expected == found
}
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Unit => "unit".into(),
        Type::Number => "number".into(),
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::UI => "ui".into(),
        Type::UIChild => "ui-child".into(),
        Type::UIChildren => "ui-children".into(),
        Type::Slot => "slot".into(),
        Type::Struct(n) | Type::Enum(n) => n.clone(),
        Type::Fn(_, _) => "function".into(),
        Type::Array(_) => "array".into(),
        Type::Tuple(_) => "tuple".into(),
        Type::Map(_) => "map".into(),
        Type::Unknown => "unknown".into(),
        other => format!("{:?}", other),
    }
}
fn init_field_parts(field: &StructInitField) -> (&str, &Expr, &Span) {
    match field {
        StructInitField::Assign { name, expr, span }
        | StructInitField::Tint { name, expr, span } => (name, expr, span),
    }
}
