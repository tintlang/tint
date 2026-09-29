/// What a method call on a receiver of a given type looks like.
enum MethodSig {
    /// Parameter types (receiver excluded) and result type.
    Known(Vec<Type>, Type),
    /// The receiver is not known yet; try again later.
    Pending,
    /// Not a built-in or user method; the name may still be a function-typed field.
    NotAMethod,
    /// Already reported.
    Failed,
}

impl SemanticChecker {
    fn visit_expr(&mut self, expr: &Expr) {
        let _ = self.infer_expr(expr);
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        self.infer_expr_with(expr, None)
    }

    fn record(&mut self, expr: &Expr, ty: &Type) {
        self.inferred.insert(
            (expr.span().start.offset, expr.span().end.offset),
            TypedExpr {
                span: expr.span(),
                ty: ty.clone(),
            },
        );
        self.inferred_by_ptr
            .insert(expr as *const Expr as usize, ty.clone());
    }

    /// `expected` is only a hint that lets lambdas learn their parameter
    /// types from where they are passed; the caller still unifies.
    fn infer_expr_with(&mut self, expr: &Expr, expected: Option<&Type>) -> Type {
        let ty = self.infer_expr_inner(expr, expected);
        self.record(expr, &ty);
        ty
    }

    fn infer_args(&mut self, args: &[Expr], params: &[Type]) -> Vec<Type> {
        args.iter()
            .enumerate()
            .map(|(index, arg)| self.infer_expr_with(arg, params.get(index)))
            .collect()
    }

    fn infer_expr_inner(&mut self, expr: &Expr, expected: Option<&Type>) -> Type {
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
            } => {
                let recv = self.infer_expr(target);
                match self.field_of(&recv, field, *span) {
                    Some(ty) => ty,
                    None => {
                        let ret = self.fresh(*span, format!("field `{field}`"));
                        self.deferred.push(Deferred::Field {
                            recv,
                            field: field.clone(),
                            ret: ret.clone(),
                            span: *span,
                        });
                        ret
                    }
                }
            }
            Expr::Array { items, span } => {
                let hint = match expected.map(|t| self.shallow(t)) {
                    Some(Type::Array(item)) => Some(*item),
                    _ => None,
                };
                let mut item_ty = match items.len() {
                    0 => hint.unwrap_or_else(|| self.fresh_hole(*span, "the element type of `[]`")),
                    _ => hint.unwrap_or_else(|| self.fresh(*span, "an array element")),
                };
                for item in items {
                    let found = self.infer_expr_with(item, Some(&item_ty));
                    self.require_compatible(&item_ty, &found, item.span());
                }
                if let Type::Var(_) = item_ty {
                    item_ty = self.shallow(&item_ty);
                }
                Type::Array(Box::new(item_ty))
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let recv = self.infer_expr(target);
                let index_ty = self.infer_expr(index);
                match self.index_of(&recv, &index_ty, *span) {
                    Some(ty) => ty,
                    None => {
                        let ret = self.fresh(*span, "an indexed element");
                        self.deferred.push(Deferred::Index {
                            recv,
                            index: index_ty,
                            ret: ret.clone(),
                            span: *span,
                        });
                        ret
                    }
                }
            }
            Expr::Unary { op, expr, span } => {
                let ty = self.infer_expr(expr);
                if op == "!" {
                    self.require_compatible(&Type::Bool, &ty, *span);
                    Type::Bool
                } else {
                    match self.shallow(&ty) {
                        // Negation keeps the operand's type; a still-open
                        // operand is constrained by whatever else uses it.
                        Type::Var(_) => ty,
                        found => {
                            self.require_compatible(&Type::Number, &found, *span);
                            if found.is_sized_numeric() {
                                found
                            } else {
                                Type::Number
                            }
                        }
                    }
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
                    _ => match self.arith_type(op, &l, &r, (left.span(), right.span()), false) {
                        Some(ty) => ty,
                        None => {
                            let ret = self.fresh(*span, format!("the result of `{op}`"));
                            self.deferred.push(Deferred::Arith {
                                op: op.clone(),
                                left: l,
                                right: r,
                                ret: ret.clone(),
                                spans: (left.span(), right.span()),
                            });
                            ret
                        }
                    },
                }
            }
            Expr::Paren(inner, _) => self.infer_expr_with(inner, expected),
            Expr::Try { expr, span } => {
                let recv = self.infer_expr(expr);
                match self.try_of(&recv, *span) {
                    Some(ty) => ty,
                    None => {
                        let ret = self.fresh(*span, "the value of `?`");
                        self.deferred.push(Deferred::Try {
                            recv,
                            ret: ret.clone(),
                            span: *span,
                        });
                        ret
                    }
                }
            }
            Expr::Cast { expr, ty, span } => {
                let source = self.infer_expr(expr);
                let target = self.ast_type(Some(ty));
                let source_now = self.shallow(&source);
                if let Type::Var(_) = source_now {
                    self.unify(&Type::Number, &source_now);
                }
                let source = self.shallow(&source);
                let numeric = |value: &Type| match value {
                    Type::Number => true,
                    Type::Simple(name) => is_numeric_name(name) || name == "number",
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
                scrutinee, arms, span,
            } => {
                let scrutinee_ty = self.infer_expr(scrutinee);
                let result = self.fresh_hole(*span, "the value of a `match`");
                for arm in arms {
                    self.scopes.push();
                    self.bind_pattern_typed(&arm.pattern, scrutinee_ty.clone());
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.infer_expr(guard);
                        self.require_compatible(&Type::Bool, &guard_ty, guard.span());
                    }
                    let arm_ty = self.infer_expr_with(&arm.expr, expected);
                    self.require_compatible(&result, &arm_ty, arm.expr.span());
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
            Expr::Lambda { params, body, span } => {
                let (hint_ret, hint_params) = match expected.map(|t| self.shallow(t)) {
                    Some(Type::Fn(ret, hint)) if hint.len() == params.len() => (Some(*ret), Some(hint)),
                    _ => (None, None),
                };
                self.scopes.push();
                let mut param_types = Vec::new();
                for (index, param) in params.iter().enumerate() {
                    let ty = match &hint_params {
                        Some(hint) => hint[index].clone(),
                        None => self.fresh(*span, format!("parameter `{param}` of a lambda")),
                    };
                    self.scopes.define(param, ty.clone());
                    param_types.push(ty);
                }
                let ret = hint_ret.unwrap_or_else(|| self.fresh(*span, "the result of a lambda"));
                let previous = self.current_return.replace(ret.clone());
                let found = self.infer_expr_with(body, Some(&ret));
                self.current_return = previous;
                self.require_compatible(&ret, &found, body.span());
                self.scopes.pop();
                Type::Fn(Box::new(ret), param_types)
            }
            Expr::StructInit { name, fields, span } => {
                let generic_params = self
                    .generic_params_by_type
                    .get(name)
                    .cloned()
                    .unwrap_or_default();
                let substitutions: HashMap<String, Type> = generic_params
                    .iter()
                    .map(|p| (p.clone(), self.fresh_hole(*span, format!("type parameter `{p}` of `{name}`"))))
                    .collect();
                if let Some(declared) = self.struct_fields.get(name).cloned() {
                    let mut seen = HashSet::new();
                    for field in fields {
                        let (field_name, value, field_span) = init_field_parts(field);
                        let want = declared
                            .get(field_name)
                            .map(|ty| substitute_type(ty, &substitutions));
                        let found = self.infer_expr_with(value, want.as_ref());
                        if let Some(want) = want {
                            self.require_compatible(&want, &found, *field_span);
                            seen.insert(field_name.to_string());
                        } else {
                            self.error(
                                *field_span,
                                SemanticErrorKind::UnknownIdent(format!("{}.{}", name, field_name)),
                            );
                        }
                    }
                    for field_name in declared.keys() {
                        if !seen.contains(field_name)
                            && !self
                                .untyped_fields
                                .contains(&(name.clone(), field_name.clone()))
                        {
                            self.error(*span, SemanticErrorKind::Unsupported);
                        }
                    }
                } else {
                    for field in fields {
                        self.visit_struct_init_field(field);
                    }
                    if !self.type_names.contains(name) && name != "Vec2" {
                        self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                    }
                }
                if generic_params.is_empty() {
                    Type::Struct(name.clone())
                } else {
                    Type::Generic(
                        name.clone(),
                        generic_params.iter().map(|p| substitutions[p].clone()).collect(),
                    )
                }
            }
            Expr::StructUpdate { base, updates, .. } => {
                let ty = self.infer_expr(base);
                let declared = match self.shallow(&ty) {
                    Type::Struct(name) => self.struct_fields.get(&name).cloned(),
                    _ => None,
                };
                for field in updates {
                    let (field_name, value, field_span) = init_field_parts(field);
                    let want = declared.as_ref().and_then(|d| d.get(field_name)).cloned();
                    let found = self.infer_expr_with(value, want.as_ref());
                    if let Some(want) = want {
                        self.require_compatible(&want, &found, *field_span);
                    }
                }
                ty
            }
            Expr::NamedArg { value, .. } => self.infer_expr_with(value, expected),
            Expr::Block(block, _) => self.infer_block_type(block),
            Expr::Tuple { items, .. } => {
                let hints = match expected.map(|t| self.shallow(t)) {
                    Some(Type::Tuple(hints)) if hints.len() == items.len() => hints,
                    _ => Vec::new(),
                };
                Type::Tuple(
                    items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| self.infer_expr_with(item, hints.get(i)))
                        .collect(),
                )
            }
            Expr::TupleIndex {
                target,
                index,
                span,
            } => {
                let recv = self.infer_expr(target);
                match self.tuple_index_of(&recv, *index, *span) {
                    Some(ty) => ty,
                    None => {
                        let ret = self.fresh(*span, "a tuple element");
                        self.deferred.push(Deferred::TupleIndex {
                            recv,
                            index: *index,
                            ret: ret.clone(),
                            span: *span,
                        });
                        ret
                    }
                }
            }
            Expr::VariantInit {
                enum_name,
                variant,
                fields,
                span,
            } => {
                if self
                    .enum_variants
                    .contains_key(&(enum_name.clone(), variant.clone()))
                {
                    let (enum_ty, declared) = self.instantiate_variant(enum_name, variant, *span);
                    let names = self
                        .variant_fields
                        .get(&(enum_name.clone(), variant.clone()))
                        .cloned()
                        .unwrap_or_default();
                    if declared.len() != fields.len() {
                        self.error(*span, SemanticErrorKind::Unsupported);
                    }
                    for (index, field) in fields.iter().enumerate() {
                        let (field_name, value, field_span) = init_field_parts(field);
                        // Named fields go to the field of that name; anything
                        // else is positional.
                        let slot = names
                            .iter()
                            .position(|n| n.as_deref() == Some(field_name))
                            .unwrap_or(index);
                        let want = declared.get(slot).cloned();
                        let found = self.infer_expr_with(value, want.as_ref());
                        if let Some(want) = want {
                            self.require_compatible(&want, &found, *field_span);
                        }
                    }
                    enum_ty
                } else {
                    self.error(
                        *span,
                        SemanticErrorKind::UnknownIdent(format!("{}::{}", enum_name, variant)),
                    );
                    for field in fields {
                        self.visit_struct_init_field(field);
                    }
                    Type::Unknown
                }
            }
            Expr::MapInit { entries, span } => {
                let hint = match expected.map(|t| self.shallow(t)) {
                    Some(Type::Map(item)) => Some(*item),
                    _ => None,
                };
                let item = match entries.len() {
                    0 => hint.unwrap_or_else(|| self.fresh_hole(*span, "the value type of `{}`")),
                    _ => hint.unwrap_or_else(|| self.fresh(*span, "a map value")),
                };
                for (_, value) in entries {
                    let found = self.infer_expr_with(value, Some(&item));
                    self.require_compatible(&item, &found, value.span());
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
            Expr::Namespace { base, item, span } => {
                if let Expr::Ident(enum_name, _) = base.as_ref() {
                    if self
                        .enum_variants
                        .contains_key(&(enum_name.clone(), item.clone()))
                    {
                        let (enum_ty, fields) = self.instantiate_variant(enum_name, item, *span);
                        return if fields.is_empty() {
                            enum_ty
                        } else {
                            Type::Fn(Box::new(enum_ty), fields)
                        };
                    }
                }
                self.infer_expr(base);
                Type::Unknown
            }
        }
    }

    /// Fresh instance of an enum variant: the enum type (with one variable per
    /// generic parameter) and the variant's field types.
    fn instantiate_variant(&mut self, enum_name: &str, variant: &str, span: Span) -> (Type, Vec<Type>) {
        let params = self
            .generic_params_by_type
            .get(enum_name)
            .cloned()
            .unwrap_or_default();
        let vars: Vec<Type> = params
            .iter()
            .map(|p| self.fresh_hole(span, format!("type parameter `{p}` of `{enum_name}`")))
            .collect();
        let substitutions: HashMap<String, Type> =
            params.iter().cloned().zip(vars.iter().cloned()).collect();
        let fields = self
            .enum_variants
            .get(&(enum_name.to_string(), variant.to_string()))
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|ty| substitute_type(ty, &substitutions))
            .collect();
        let ty = if params.is_empty() {
            Type::Enum(enum_name.to_string())
        } else {
            Type::Generic(enum_name.to_string(), vars)
        };
        (ty, fields)
    }

    /// The enum a bare variant name (`Some`, `Circle`, ...) belongs to, when
    /// exactly one enum has a variant of that name.
    fn enum_of_variant(&self, variant: &str) -> Option<String> {
        let mut owners = self
            .enum_variants
            .keys()
            .filter(|(_, name)| name == variant)
            .map(|(owner, _)| owner.clone());
        let first = owners.next()?;
        owners.next().is_none().then_some(first)
    }

    fn field_of(&mut self, recv: &Type, field: &str, span: Span) -> Option<Type> {
        match self.shallow(recv) {
            Type::Var(_) => None,
            Type::Unknown => Some(Type::Unknown),
            Type::Struct(name) => Some(
                self.struct_fields
                    .get(&name)
                    .and_then(|f| f.get(field))
                    .cloned()
                    .unwrap_or_else(|| {
                        self.error(
                            span,
                            SemanticErrorKind::UnknownIdent(format!("{}.{}", name, field)),
                        );
                        Type::Unknown
                    }),
            ),
            Type::Generic(name, args) if self.struct_fields.contains_key(&name) => {
                let params = self.generic_params_by_type.get(&name).cloned().unwrap_or_default();
                let substitutions: HashMap<String, Type> = params.into_iter().zip(args).collect();
                Some(
                    self.struct_fields
                        .get(&name)
                        .and_then(|f| f.get(field))
                        .map(|ty| substitute_type(ty, &substitutions))
                        .unwrap_or_else(|| {
                            self.error(
                                span,
                                SemanticErrorKind::UnknownIdent(format!("{}.{}", name, field)),
                            );
                            Type::Unknown
                        }),
                )
            }
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "struct".into(),
                        found: type_name(&self.resolve(&found)),
                    },
                );
                Some(Type::Unknown)
            }
        }
    }

    fn index_of(&mut self, recv: &Type, index: &Type, span: Span) -> Option<Type> {
        match self.shallow(recv) {
            Type::Var(_) => None,
            Type::Unknown => Some(Type::Unknown),
            Type::Array(item) => {
                self.require_compatible(&Type::Number, index, span);
                Some(*item)
            }
            Type::Map(item) => Some(*item),
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "array or map".into(),
                        found: type_name(&self.resolve(&found)),
                    },
                );
                Some(Type::Unknown)
            }
        }
    }

    fn tuple_index_of(&mut self, recv: &Type, index: usize, span: Span) -> Option<Type> {
        match self.shallow(recv) {
            Type::Var(_) => None,
            Type::Unknown => Some(Type::Unknown),
            Type::Tuple(items) => Some(items.get(index).cloned().unwrap_or_else(|| {
                self.error(span, SemanticErrorKind::Unsupported);
                Type::Unknown
            })),
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "tuple".into(),
                        found: type_name(&self.resolve(&found)),
                    },
                );
                Some(Type::Unknown)
            }
        }
    }

    fn try_of(&mut self, recv: &Type, span: Span) -> Option<Type> {
        match self.shallow(recv) {
            Type::Var(_) => None,
            Type::Unknown => Some(Type::Unknown),
            Type::Generic(name, args) if name == "Option" || name == "Result" => {
                // `Err(e)?` returns the error as is, so the enclosing function
                // has to return a `Result` with the same error type.
                if name == "Result" && args.len() == 2 {
                    let enclosing = self.current_return.clone().map(|ty| self.shallow(&ty));
                    if let Some(Type::Generic(outer, outer_args)) = enclosing {
                        if outer == "Result" && outer_args.len() == 2 {
                            self.require_compatible(&outer_args[1], &args[1], span);
                        }
                    }
                }
                Some(args.into_iter().next().unwrap_or(Type::Unknown))
            }
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "Option or Result".into(),
                        found: type_name(&self.resolve(&found)),
                    },
                );
                Some(Type::Unknown)
            }
        }
    }

    /// Type of `left op right` for `+ - * / %`. `None` while an operand is
    /// still open, unless `last_call` is set: then open operands become `number`.
    fn arith_type(&mut self, op: &str, left: &Type, right: &Type, spans: (Span, Span), last_call: bool) -> Option<Type> {
        let (mut l, mut r) = (self.shallow(left), self.shallow(right));
        let text = |t: &Type| matches!(t, Type::String);
        // String `+` appends the other side's display text, so numbers and
        // bools are fine next to a string; lists, structs and the like are not.
        if op == "+" && (text(&l) || text(&r)) {
            for (ty, span) in [(&l, spans.0), (&r, spans.1)] {
                if !matches!(
                    ty,
                    Type::String | Type::Number | Type::Bool | Type::Unknown | Type::Simple(_) | Type::Var(_)
                ) {
                    self.require_compatible(&Type::String, ty, span);
                }
            }
            return Some(Type::String);
        }
        if matches!(l, Type::Var(_)) || matches!(r, Type::Var(_)) {
            if !last_call {
                return None;
            }
            self.unify(&Type::Number, &l);
            self.unify(&Type::Number, &r);
            l = self.shallow(&l);
            r = self.shallow(&r);
        }
        self.require_compatible(&Type::Number, &l, spans.0);
        self.require_compatible(&Type::Number, &r, spans.1);
        Some(if l.is_sized_numeric() {
            l
        } else if r.is_sized_numeric() {
            r
        } else {
            Type::Number
        })
    }

    fn infer_call(&mut self, target: &Expr, args: &[Expr], span: Span) -> Type {
        // `Enum::Variant(args)`, or a bare `Some(x)` / `Ok(v)`.
        let variant = match target {
            Expr::Namespace { base, item, .. } => match base.as_ref() {
                Expr::Ident(enum_name, _)
                    if self.enum_variants.contains_key(&(enum_name.clone(), item.clone())) =>
                {
                    Some((enum_name.clone(), item.clone()))
                }
                _ => None,
            },
            Expr::Ident(name, _)
                if self.scopes.lookup(name).is_none() && !self.fn_types.contains_key(name) =>
            {
                self.enum_of_variant(name).map(|owner| (owner, name.clone()))
            }
            _ => None,
        };
        if let Some((enum_name, variant)) = variant {
            let (enum_ty, fields) = self.instantiate_variant(&enum_name, &variant, span);
            let arg_types = self.infer_args(args, &fields);
            self.check_args(&fields, &arg_types, span);
            let callee = if fields.is_empty() { enum_ty.clone() } else { Type::Fn(Box::new(enum_ty.clone()), fields) };
            self.record(target, &callee);
            return enum_ty;
        }

        if let Expr::Field {
            target: receiver,
            field: method,
            ..
        } = target
        {
            let recv = self.infer_expr(receiver);
            return match self.method_sig(&recv, method, args.len(), span) {
                MethodSig::Known(params, ret) => {
                    let arg_types = self.infer_args(args, &params);
                    self.check_method_args(&recv, method, &params, &arg_types, span);
                    self.record(target, &Type::Fn(Box::new(ret.clone()), params));
                    ret
                }
                MethodSig::Pending => {
                    let arg_types = self.infer_args(args, &[]);
                    let ret = self.fresh(span, format!("the result of `.{method}(..)`"));
                    self.record(target, &Type::Fn(Box::new(ret.clone()), arg_types.clone()));
                    self.deferred.push(Deferred::Method {
                        recv,
                        method: method.clone(),
                        args: arg_types,
                        ret: ret.clone(),
                        span,
                    });
                    ret
                }
                MethodSig::Failed => {
                    self.infer_args(args, &[]);
                    Type::Unknown
                }
                MethodSig::NotAMethod => {
                    // A struct field holding a function.
                    let callee = self.field_of(&recv, method, span).unwrap_or(Type::Unknown);
                    self.record(target, &callee);
                    self.call_with(&callee, args, span)
                }
            };
        }

        if let Expr::Ident(name, id_span) = target {
            if let Some(local) = self.scopes.lookup(name) {
                self.references.push(Reference { name: name.clone(), span: *id_span });
                self.record(target, &local);
                return self.call_with(&local, args, span);
            }
            if let Some((params, ret)) = self.fn_types.get(name).cloned() {
                let arg_types = self.infer_args(args, &params);
                self.check_call_args(&(String::new(), name.clone()), &params, &arg_types, span);
                self.record(target, &Type::Fn(Box::new(ret.clone()), params));
                return ret;
            }
            if let Some(ty) = self.infer_builtin_call(name, args, span) {
                return ty;
            }
            if !self.known_fns.contains(name) {
                self.error(span, SemanticErrorKind::UnknownIdent(name.clone()));
            }
            self.infer_args(args, &[]);
            return if self.struct_fields.contains_key(name) {
                Type::Struct(name.clone())
            } else if self.ui_fn_names.contains(name) {
                Type::UI
            } else {
                Type::Unknown
            };
        }

        let callee = self.infer_expr(target);
        self.call_with(&callee, args, span)
    }

    /// Calls through a value of function type (or a still-open type).
    fn call_with(&mut self, callee: &Type, args: &[Expr], span: Span) -> Type {
        let hints = match self.shallow(callee) {
            Type::Fn(_, params) => params,
            _ => Vec::new(),
        };
        let arg_types = self.infer_args(args, &hints);
        self.call_type(callee, &arg_types, span)
    }

    fn call_type(&mut self, callee: &Type, arg_types: &[Type], span: Span) -> Type {
        match self.shallow(callee) {
            Type::Fn(ret, params) => {
                self.check_args(&params, arg_types, span);
                *ret
            }
            Type::Var(_) => {
                let ret = self.fresh(span, "the result of a call");
                let shape = Type::Fn(Box::new(ret.clone()), arg_types.to_vec());
                self.require_compatible(callee, &shape, span);
                ret
            }
            Type::Unknown => Type::Unknown,
            found => {
                self.error(
                    span,
                    SemanticErrorKind::TypeMismatch {
                        expected: "function".into(),
                        found: type_name(&self.resolve(&found)),
                    },
                );
                Type::Unknown
            }
        }
    }

    fn infer_builtin_call(&mut self, name: &str, args: &[Expr], span: Span) -> Option<Type> {
        let (params, ret): (Option<Vec<Type>>, Type) = match name {
            "vec2" => (
                Some(vec![Type::Number, Type::Number]),
                Type::Struct("Vec2".into()),
            ),
            "clamp" | "min" | "max" | "abs" | "sign" | "sqrt" => (None, Type::Number),
            "parse_number" => (
                Some(vec![Type::String]),
                Type::Generic("Result".into(), vec![Type::Number, Type::String]),
            ),
            "read_line" | "read_key" => (Some(vec![]), Type::String),
            "print" | "println" | "log" | "dbg" | "debug" | "error" => (None, Type::Unit),
            _ => return None,
        };
        let arg_types = self.infer_args(args, params.as_deref().unwrap_or(&[]));
        if let Some(params) = params {
            if name == "vec2" {
                self.check_args(&params, &arg_types, span);
            }
        }
        Some(ret)
    }

    /// Signature of `receiver.method(..)`: the built-in methods of lists,
    /// strings, maps, `Option` and `Result`, or a user method of a struct.
    fn method_sig(&mut self, receiver: &Type, method: &str, nargs: usize, span: Span) -> MethodSig {
        let receiver = self.shallow(receiver);
        match &receiver {
            Type::Var(_) => return MethodSig::Pending,
            Type::Unknown => return MethodSig::Failed,
            Type::Array(_) | Type::String | Type::Map(_) => {
                return match self.collection_method_sig(&receiver, method, nargs, span) {
                    Some((params, ret)) => MethodSig::Known(params, ret),
                    None => MethodSig::Failed,
                };
            }
            Type::Struct(name) => {
                return match self.methods.get(&(name.clone(), method.to_string())).cloned() {
                    Some((params, ret)) => MethodSig::Known(params, ret),
                    None => MethodSig::NotAMethod,
                };
            }
            Type::Generic(name, args) if name == "Option" || name == "Result" => {
                let is_option = name == "Option";
                let t = args.first().cloned().unwrap_or(Type::Unknown);
                let e = args.get(1).cloned().unwrap_or(Type::Unknown);
                let generic = |n: &str, items: Vec<Type>| Type::Generic(n.into(), items);
                // The same kind of value with another payload type.
                let with = |payload: Type| {
                    if is_option {
                        generic("Option", vec![payload])
                    } else {
                        generic("Result", vec![payload, e.clone()])
                    }
                };
                let func = |ret: Type, params: Vec<Type>| Type::Fn(Box::new(ret), params);
                // The payload the "failure" side hands to a fallback closure.
                let fail_params = if is_option { vec![] } else { vec![e.clone()] };
                let known = |params: Vec<Type>, ret: Type| MethodSig::Known(params, ret);
                match (is_option, method) {
                    (_, "unwrap") => return known(vec![], t),
                    (_, "expect") => return known(vec![Type::String], t),
                    (true, "is_some" | "is_none") | (false, "is_ok" | "is_err") => {
                        return known(vec![], Type::Bool)
                    }
                    (true, "is_some_and") | (false, "is_ok_and") => {
                        return known(vec![func(Type::Bool, vec![t])], Type::Bool)
                    }
                    (false, "is_err_and") => return known(vec![func(Type::Bool, vec![e])], Type::Bool),
                    (_, "unwrap_or") => return known(vec![t.clone()], t),
                    (_, "unwrap_or_else") => return known(vec![func(t.clone(), fail_params)], t),
                    (_, "map") => {
                        let mapped = self.fresh(span, "the result of `map`");
                        return known(vec![func(mapped.clone(), vec![t])], with(mapped));
                    }
                    (_, "map_or") => {
                        let out = self.fresh(span, "the result of `map_or`");
                        return known(vec![out.clone(), func(out.clone(), vec![t])], out);
                    }
                    (_, "and_then") => {
                        let mapped = self.fresh(span, "the result of `and_then`");
                        return known(vec![func(with(mapped.clone()), vec![t])], with(mapped));
                    }
                    (true, "filter") => {
                        return known(vec![func(Type::Bool, vec![t.clone()])], with(t))
                    }
                    (true, "or") => return known(vec![with(t.clone())], with(t)),
                    (true, "or_else") => return known(vec![func(with(t.clone()), vec![])], with(t)),
                    (true, "ok_or") => {
                        let err = self.fresh(span, "the error of `ok_or`");
                        return known(vec![err.clone()], generic("Result", vec![t, err]));
                    }
                    (true, "ok_or_else") => {
                        let err = self.fresh(span, "the error of `ok_or_else`");
                        return known(
                            vec![func(err.clone(), vec![])],
                            generic("Result", vec![t, err]),
                        );
                    }
                    (false, "or") => {
                        let other = self.fresh(span, "the error of `or`");
                        let out = generic("Result", vec![t, other]);
                        return known(vec![out.clone()], out);
                    }
                    (false, "or_else") => {
                        let other = self.fresh(span, "the error of `or_else`");
                        let out = generic("Result", vec![t, other]);
                        return known(vec![func(out.clone(), vec![e])], out);
                    }
                    (false, "map_err") => {
                        let mapped = self.fresh(span, "the result of `map_err`");
                        return known(
                            vec![func(mapped.clone(), vec![e])],
                            generic("Result", vec![t, mapped]),
                        );
                    }
                    (false, "ok") => return known(vec![], generic("Option", vec![t])),
                    (false, "err") => return known(vec![], generic("Option", vec![e])),
                    (false, "unwrap_err") => return known(vec![], e),
                    (false, "expect_err") => return known(vec![Type::String], e),
                    _ => return MethodSig::NotAMethod,
                }
            }
            _ => {}
        }
        MethodSig::NotAMethod
    }

    /// Like `check_args` for a call of a known function or method: a call
    /// whose argument disagrees with an unannotated parameter does not fail,
    /// it makes the parameter polymorphic (see `polymorphic`).
    fn check_call_args(&mut self, key: &(String, String), params: &[Type], args: &[Type], span: Span) {
        if params.len() != args.len() {
            self.error(span, SemanticErrorKind::Unsupported);
        }
        for (index, (expected, found)) in params.iter().zip(args).enumerate() {
            if self.unify(expected, found) {
                continue;
            }
            let open = self.unannotated.get(key).and_then(|f| f.get(index)).copied().unwrap_or(false);
            if open {
                let list = self.polymorphic.entry(key.clone()).or_default();
                if !list.contains(&index) {
                    list.push(index);
                }
            } else {
                self.require_compatible(expected, found, span);
            }
        }
    }

    fn check_method_args(&mut self, recv: &Type, method: &str, params: &[Type], args: &[Type], span: Span) {
        match self.shallow(recv) {
            Type::Struct(name) => {
                self.check_call_args(&(name, method.to_string()), params, args, span)
            }
            _ => self.check_args(params, args, span),
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
        Type::Unknown | Type::Var(_) => "unknown".into(),
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
        Type::Map(item) => Type::Map(Box::new(substitute_type(item, substitutions))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(|item| substitute_type(item, substitutions)).collect()),
        Type::Fn(ret, params) => Type::Fn(
            Box::new(substitute_type(ret, substitutions)),
            params.iter().map(|param| substitute_type(param, substitutions)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn init_field_parts(field: &StructInitField) -> (&str, &Expr, &Span) {
    match field {
        StructInitField::Assign { name, expr, span }
        | StructInitField::Tint { name, expr, span } => (name, expr, span),
    }
}
