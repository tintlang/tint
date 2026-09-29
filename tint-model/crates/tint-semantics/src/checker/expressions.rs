impl SemanticChecker {
    fn visit_expr(&mut self, expr: &Expr) {
        let _ = self.infer_expr(expr);
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        let ty = self.infer_expr_inner(expr);
        self.inferred.insert(
            (expr.span().start.offset, expr.span().end.offset),
            TypedExpr {
                span: expr.span(),
                ty: ty.clone(),
            },
        );
        ty
    }

    fn infer_expr_inner(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Number(_, _) => Type::Number,
            Expr::String(_, _) => Type::String,
            Expr::Bool(_, _) => Type::Bool,
            Expr::Unit(_) => Type::Unit,
            Expr::Ident(id, span) => {
                self.references.push(Reference {
                    name: id.clone(),
                    span: *span,
                });
                self.scopes
                    .lookup(id)
                    .or_else(|| {
                        self.fn_types
                            .get(id)
                            .map(|(params, ret)| Type::Fn(Box::new(ret.clone()), params.clone()))
                    })
                    .unwrap_or_else(|| {
                        self.error(*span, SemanticErrorKind::UnknownIdent(id.clone()));
                        Type::Unknown
                    })
            }
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
                    // String `+` appends the other side's display text, so
                    // numbers and bools are fine next to a string; lists,
                    // structs and the like are not.
                    "+" if matches!(l, Type::String) || matches!(r, Type::String) => {
                        for (ty, side) in [(&l, left.span()), (&r, right.span())] {
                            if !matches!(
                                ty,
                                Type::String
                                    | Type::Number
                                    | Type::Bool
                                    | Type::Unknown
                                    | Type::Simple(_)
                            ) {
                                self.require_compatible(&Type::String, ty, side);
                            }
                        }
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
            Expr::Try { expr, span } => {
                match self.infer_expr(expr) {
                    Type::Generic(name, args) if name == "Option" || name == "Result" =>
                        args.into_iter().next().unwrap_or(Type::Unknown),
                    found => {
                        self.error(
                            *span,
                            SemanticErrorKind::TypeMismatch {
                                expected: "Option or Result".into(),
                                found: type_name(&found),
                            },
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::Cast { expr, ty, span } => {
                let source = self.infer_expr(expr);
                let target = self.ast_type(Some(ty));
                let numeric = |value: &Type| match value {
                    Type::Number => true,
                    Type::Simple(name) => matches!(name.as_str(), "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64" | "number"),
                    _ => false,
                };
                if !numeric(&source) || !numeric(&target) {
                    self.error(*span, SemanticErrorKind::TypeMismatch { expected: "numeric type".into(), found: type_name(&source) });
                    Type::Unknown
                } else {
                    target
                }
            }
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
            Expr::If {
                cond, then, else_, ..
            } => {
                let cond_ty = self.infer_expr(cond);
                self.require_compatible(&Type::Bool, &cond_ty, cond.span());
                let then_ty = self.infer_block_type(then);
                let else_ty = self.infer_block_type(else_);
                self.require_compatible(&then_ty, &else_ty, else_.span);
                then_ty
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
                let generic_params = self
                    .generic_params_by_type
                    .get(name)
                    .cloned()
                    .unwrap_or_default();
                let mut substitutions = HashMap::new();
                if let Some(expected) = self.struct_fields.get(name).cloned() {
                    let mut seen = HashSet::new();
                    for field in fields {
                        let (field_name, value, field_span) = init_field_parts(field);
                        let found = self.infer_expr(value);
                        if let Some(field_ty) = expected.get(field_name) {
                            if let Type::Simple(parameter) = field_ty {
                                if generic_params.iter().any(|name| name == parameter) {
                                    substitutions.insert(parameter.clone(), found.clone());
                                }
                            }
                            let expected_ty = substitute_type(field_ty, &substitutions);
                            self.require_compatible(&expected_ty, &found, *field_span);
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
                if generic_params.is_empty() {
                    Type::Struct(name.clone())
                } else {
                    Type::Generic(
                        name.clone(),
                        generic_params
                            .iter()
                            .map(|parameter| substitutions.get(parameter).cloned().unwrap_or(Type::Unknown))
                            .collect(),
                    )
                }
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
                let generic_params = self
                    .generic_params_by_type
                    .get(enum_name)
                    .cloned()
                    .unwrap_or_default();
                let mut substitutions = HashMap::new();
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
                            if let Type::Simple(parameter) = expected_ty {
                                if generic_params.iter().any(|name| name == parameter) {
                                    substitutions.insert(parameter.clone(), found.clone());
                                }
                            }
                            let expected_ty = substitute_type(expected_ty, &substitutions);
                            self.require_compatible(&expected_ty, &found, *field_span);
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
                if enum_name == "Option" || enum_name == "Result" {
                    let payloads = fields
                        .iter()
                        .map(|field| self.infer_expr(init_field_parts(field).1))
                        .collect::<Vec<_>>();
                    match (enum_name.as_str(), variant.as_str()) {
                        ("Option", "Some") => Type::Generic(
                            "Option".into(),
                            vec![payloads.first().cloned().unwrap_or(Type::Unknown)],
                        ),
                        ("Option", "None") => {
                            Type::Generic("Option".into(), vec![Type::Unknown])
                        }
                        ("Result", "Ok") => Type::Generic(
                            "Result".into(),
                            vec![payloads.first().cloned().unwrap_or(Type::Unknown), Type::Unknown],
                        ),
                        ("Result", "Err") => Type::Generic(
                            "Result".into(),
                            vec![Type::Unknown, payloads.first().cloned().unwrap_or(Type::Unknown)],
                        ),
                        _ => Type::Enum(enum_name.clone()),
                    }
                } else if generic_params.is_empty() {
                    Type::Enum(enum_name.clone())
                } else {
                    Type::Generic(
                        enum_name.clone(),
                        generic_params
                            .iter()
                            .map(|parameter| substitutions.get(parameter).cloned().unwrap_or(Type::Unknown))
                            .collect(),
                    )
                }
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
                    return builtin_variant_type(enum_name, item, &arg_types);
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
            if let Some(ty) = self.infer_collection_method(&receiver_ty, method, &arg_types, span) {
                return ty;
            }
            if let Type::Struct(name) = receiver_ty {
                if let Some((params, ret)) = self.methods.get(&(name.clone(), method.clone())).cloned() {
                    if self.record_calls {
                        self.call_args
                            .entry((name, method.clone()))
                            .or_default()
                            .push(arg_types.clone());
                    }
                    self.check_args(&params, &arg_types, span);
                    return ret;
                }
            } else {
                let (name, args) = match receiver_ty {
                    Type::Generic(name, args) => (name, args),
                    Type::Enum(name) => (name, Vec::new()),
                    _ => (String::new(), Vec::new()),
                };
                if name == "Option" || name == "Result" {
                    match method.as_str() {
                        "unwrap" => {
                            if !arg_types.is_empty() {
                                self.error(span, SemanticErrorKind::Unsupported);
                            }
                            return args.first().cloned().unwrap_or(Type::Unknown);
                        }
                        "expect" => {
                            self.check_args(&[Type::String], &arg_types, span);
                            return args.first().cloned().unwrap_or(Type::Unknown);
                        }
                        "is_some" | "is_none" | "is_ok" | "is_err" => {
                            if !arg_types.is_empty() {
                                self.error(span, SemanticErrorKind::Unsupported);
                            }
                            return Type::Bool;
                        }
                        "unwrap_or" => {
                            self.check_args(&[args.first().cloned().unwrap_or(Type::Unknown)], &arg_types, span);
                            return args.first().cloned().unwrap_or(Type::Unknown);
                        }
                        "map" => {
                            if let Some(Type::Fn(ret, _)) = arg_types.first() {
                                return Type::Generic(name, vec![(**ret).clone()]);
                            }
                            return Type::Generic(name, vec![Type::Unknown]);
                        }
                        "and_then" => {
                            if let Some(Type::Fn(ret, _)) = arg_types.first() {
                                return *ret.clone();
                            }
                            return Type::Unknown;
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Expr::Ident(name, _) = target {
            if let Some((params, ret)) = self.fn_types.get(name).cloned() {
                if self.record_calls && self.scopes.lookup(name).is_none_or(|t| matches!(t, Type::Fn(..))) {
                    self.call_args
                        .entry((String::new(), name.clone()))
                        .or_default()
                        .push(arg_types.clone());
                }
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
                "clamp" | "min" | "max" | "abs" | "sign" | "sqrt" => {
                    return Type::Number;
                }
                "parse_number" => {
                    return Type::Generic(
                        "Result".into(),
                        vec![Type::Number, Type::String],
                    );
                }
                "read_line" | "read_key" => return Type::String,
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
            ("f32" | "f64" | "i32" | "i64" | "u8" | "u32" | "u64", Type::Number)
        );
    }
    if let Type::Simple(name) = found {
        return matches!(
            (name.as_str(), expected),
            ("f32" | "f64" | "i32" | "i64" | "u8" | "u32" | "u64", Type::Number)
        );
    }
    if let (Type::Fn(expected_ret, expected_params), Type::Fn(found_ret, found_params)) =
        (expected, found)
    {
        expected_params.len() == found_params.len()
            && expected_params
                .iter()
                .zip(found_params)
                .all(|(e, f)| compatible(e, f))
            && compatible(expected_ret, found_ret)
    } else if let (Type::Generic(expected_name, expected_args), Type::Generic(found_name, found_args)) =
        (expected, found)
    {
        expected_name == found_name
            && expected_args.len() == found_args.len()
            && expected_args
                .iter()
                .zip(found_args)
                .all(|(expected, found)| compatible(expected, found))
    } else {
        expected == found
    }
}
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Simple(name) => name.clone(),
        Type::Unit => "unit".into(),
        Type::Number => "number".into(),
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::UI => "ui".into(),
        Type::UIChild => "ui-child".into(),
        Type::UIChildren => "ui-children".into(),
        Type::Slot => "slot".into(),
        Type::Struct(n) | Type::Enum(n) => n.clone(),
        Type::Generic(name, args) => format!(
            "{}<{}>",
            name,
            args.iter().map(type_name).collect::<Vec<_>>().join(", ")
        ),
        Type::Fn(_, _) => "function".into(),
        Type::Array(_) => "array".into(),
        Type::Tuple(_) => "tuple".into(),
        Type::Map(_) => "map".into(),
        Type::Unknown => "unknown".into(),
        other => format!("{:?}", other),
    }
}

fn substitute_type(ty: &Type, substitutions: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Simple(name) => substitutions.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Generic(name, args) => Type::Generic(
            name.clone(),
            args.iter().map(|arg| substitute_type(arg, substitutions)).collect(),
        ),
        Type::Array(item) => Type::Array(Box::new(substitute_type(item, substitutions))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(|item| substitute_type(item, substitutions)).collect()),
        Type::Fn(ret, params) => Type::Fn(
            Box::new(substitute_type(ret, substitutions)),
            params.iter().map(|param| substitute_type(param, substitutions)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn builtin_variant_type(enum_name: &str, variant: &str, args: &[Type]) -> Type {
    match (enum_name, variant) {
        ("Option", "Some") => Type::Generic(
            "Option".into(),
            vec![args.first().cloned().unwrap_or(Type::Unknown)],
        ),
        ("Option", "None") => Type::Generic("Option".into(), vec![Type::Unknown]),
        ("Result", "Ok") => Type::Generic(
            "Result".into(),
            vec![args.first().cloned().unwrap_or(Type::Unknown), Type::Unknown],
        ),
        ("Result", "Err") => Type::Generic(
            "Result".into(),
            vec![Type::Unknown, args.first().cloned().unwrap_or(Type::Unknown)],
        ),
        _ => Type::Enum(enum_name.into()),
    }
}
fn init_field_parts(field: &StructInitField) -> (&str, &Expr, &Span) {
    match field {
        StructInitField::Assign { name, expr, span }
        | StructInitField::Tint { name, expr, span } => (name, expr, span),
    }
}
