impl SemanticChecker {
    pub fn new(ctx: CheckerContext) -> Self {
        SemanticChecker {
            ctx,
            scopes: Scopes::new(),
            known_fns: HashSet::new(),
            host_js: false,
            ui_tokens: HashSet::new(),
            ui_styles: HashSet::new(),
            ui_variants: HashSet::new(),
            fn_types: HashMap::new(),
            fn_sigs: HashMap::new(),
            fn_generics: HashMap::new(),
            instances: HashMap::new(),
            struct_fields: HashMap::new(),
            methods: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_names: HashSet::new(),
            type_names: HashSet::new(),
            generic_arity: HashMap::new(),
            generic_params_by_type: HashMap::new(),
            generic_params: HashSet::new(),
            current_return: None,
            variant_fields: HashMap::new(),
            untyped_fields: HashSet::new(),
            ui_fn_names: HashSet::new(),
            unannotated: HashMap::new(),
            polymorphic: HashMap::new(),
            for_item: None,
            rebind_allowed: false,
            global_lets: HashMap::new(),
            subst: Vec::new(),
            var_info: Vec::new(),
            deferred: Vec::new(),
            errors: vec![],
            inferred: HashMap::new(),
            inferred_by_ptr: HashMap::new(),
            binding_types: HashMap::new(),
            references: vec![],
            symbols: vec![],
            external_context: SemanticContext::default(),
        }
    }

    /// Builds the UI/name environment exported by a fully resolved program.
    /// This is intentionally separate from `check`: an editor may check an
    /// imported fragment against the context of its entry file.
    pub fn context_for_program(program: &Program) -> SemanticContext {
        let mut checker = Self::new(Default::default());
        checker.collect_top_level_names(&program.items);
        for item in &program.items {
            if let Item::UiFn(function) = item {
                for node in &function.body {
                    checker.collect_ui_tokens(node);
                    checker.collect_ui_styles(node);
                    checker.collect_ui_variants(node);
                }
            }
        }
        SemanticContext {
            known_fns: checker.known_fns,
            ui_tokens: checker.ui_tokens,
            ui_styles: checker.ui_styles,
            ui_variants: checker.ui_variants,
        }
    }

    pub fn check_with_context(
        &mut self,
        program: &Program,
        context: &SemanticContext,
    ) -> (Vec<SemanticError>, SemanticModel) {
        self.external_context = context.clone();
        self.check_with_model(program)
    }

    pub fn check(&mut self, program: &Program) -> Vec<SemanticError> {
        self.errors.clear();
        self.inferred.clear();
        self.inferred_by_ptr.clear();
        self.instances.clear();
        self.binding_types.clear();
        self.references.clear();
        self.symbols.clear();
        self.subst.clear();
        self.var_info.clear();
        self.deferred.clear();
        self.known_fns.extend(self.external_context.known_fns.iter().cloned());
        self.known_fns.extend(self.ctx.host_fns.iter().cloned());
        let meta = program.app_meta();
        self.host_js = !meta.js.is_empty() || !meta.rs.is_empty();
        self.collect_top_level_names(&program.items);
        self.type_ui_state(&program.items);
        for item in &program.items {
            self.visit_item(item);
        }
        self.finalize_types();
        self.errors.clone()
    }

    pub fn check_with_model(&mut self, program: &Program) -> (Vec<SemanticError>, SemanticModel) {
        let errors = self.check(program);
        let mut expressions: Vec<_> = self.inferred.values().cloned().collect();
        expressions.sort_by_key(|typed| (typed.span.start.offset, typed.span.end.offset));
        let globals = self.scopes.scopes[0].clone();
        (
            errors,
            SemanticModel {
                expressions,
                symbols: self.symbols.clone(),
                references: self.references.clone(),
                by_expr: self.inferred_by_ptr.clone(),
                functions: self.fn_types.clone(),
                methods: self.methods.clone(),
                globals,
                bindings: self.binding_types.clone(),
                struct_fields: self.struct_fields.clone(),
                variant_types: self.enum_variants.clone(),
                variant_names: self.variant_fields.clone(),
                function_generics: self.fn_generics.clone(),
                instances: self.instances.clone(),
            },
        )
    }

    fn collect_top_level_names(&mut self, items: &[Item]) {
        // `viewport_width` is injected by UiSession rather than declared in
        // Tint source. It is nevertheless part of every UI function's
        // persistent runtime scope, so imported UI fragments must see it
        // during semantic checking too.
        self.scopes
            .define_with_mutability("viewport_width", Type::Number, true);
        // UiSession also provides a default `theme` value for UI trees that
        // use imported theme-switch helpers without declaring `state theme`
        // themselves (Pong is one such consumer).
        self.scopes
            .define_with_mutability("theme", Type::String, true);
        // The current URL path, kept up to date by the host (see
        // `UiSession::set_route_path`); `if{route_path == "/about"}`.
        self.scopes
            .define_with_mutability("route_path", Type::String, true);

        for builtin in [
            "print",
            "println",
            "read_line",
            "read_key",
            "parse_number",
            "log",
            "dbg",
            "debug",
            "error",
            "sqrt",
            "vec2",
            "clamp",
            "min",
            "max",
            "abs",
            "sign",
            "tint_highlight",
            "line_count",
            "max_line_len",
            "line_numbers",
            "storage_get_or",
            "storage_get",
            "storage_set",
            "storage_remove",
        ] {
            self.known_fns.insert(builtin.to_string());
        }
        self.struct_fields.insert(
            "Vec2".into(),
            HashMap::from([("x".into(), Type::Number), ("y".into(), Type::Number)]),
        );
        self.methods.insert(
            ("Vec2".into(), "add".into()),
            (
                vec![Type::Struct("Vec2".into())],
                Type::Struct("Vec2".into()),
            ),
        );
        self.methods.insert(
            ("Vec2".into(), "scale".into()),
            (vec![Type::Number], Type::Struct("Vec2".into())),
        );
        self.methods.insert(
            ("Vec2".into(), "normalized".into()),
            (vec![], Type::Struct("Vec2".into())),
        );

        // Built-in algebraic data types. Their payload types become strict
        // once generic type arguments are carried through the type table;
        // for now the checker still validates their shape and variants.
        self.type_names.insert("Option".into());
        self.type_names.insert("Result".into());
        self.enum_names.insert("Option".into());
        self.enum_names.insert("Result".into());
        self.generic_arity.insert("Option".into(), 1);
        self.generic_arity.insert("Result".into(), 2);
        self.generic_params_by_type.insert("Option".into(), vec!["T".into()]);
        self.generic_params_by_type.insert("Result".into(), vec!["T".into(), "E".into()]);
        let t = || Type::Simple("T".into());
        let e = || Type::Simple("E".into());
        for (owner, variant, payload, field) in [
            ("Option", "Some", vec![t()], Some("value")),
            ("Option", "None", vec![], None),
            ("Result", "Ok", vec![t()], Some("value")),
            ("Result", "Err", vec![e()], Some("error")),
        ] {
            let key = (owner.to_string(), variant.to_string());
            self.enum_variants.insert(key.clone(), payload);
            self.variant_fields
                .insert(key, field.map(|f| vec![Some(f.to_string())]).unwrap_or_default());
        }
        for item in items {
            match item {
                Item::Struct(s) | Item::ExportStruct(s) => {
                    self.type_names.insert(s.name.clone());
                    self.generic_arity.insert(s.name.clone(), s.generics.len());
                    self.generic_params_by_type.insert(s.name.clone(), s.generics.clone());
                }
                Item::Enum(e) | Item::ExportEnum(e) => {
                    self.type_names.insert(e.name.clone());
                    self.enum_names.insert(e.name.clone());
                    self.generic_arity.insert(e.name.clone(), e.generics.len());
                    self.generic_params_by_type.insert(e.name.clone(), e.generics.clone());
                }
                Item::TypeAlias(alias) => {
                    self.type_names.insert(alias.name.clone());
                }
                _ => {}
            };
        }
        for item in items {
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    self.generic_params.extend(f.generics.iter().cloned());
                    let params = f
                        .params
                        .iter()
                        .map(|p| self.param_type(&f.name, p))
                        .collect::<Vec<_>>();
                    let ret = match &f.ret_ty {
                        Some(t) => self.ast_type(Some(t)),
                        None => self.fresh(f.span, format!("the return type of `{}`", f.name)),
                    };
                    self.unannotated.insert(
                        (String::new(), f.name.clone()),
                        f.params.iter().map(|p| p.ty.is_none()).collect(),
                    );
                    self.symbols.push(Symbol {
                        name: f.name.clone(),
                        span: f.span,
                        ty: Type::Fn(Box::new(ret.clone()), params.clone()),
                        kind: SymbolKind::Function,
                    });
                    self.known_fns.insert(f.name.clone());
                    for param in &f.params {
                        if let Some(ty) = &param.ty {
                            self.validate_decl_type(ty);
                        }
                    }
                    if let Some(ty) = &f.ret_ty {
                        self.validate_decl_type(ty);
                    }
                    self.type_names.extend(f.generics.iter().cloned());
                    for generic in &f.generics {
                        self.generic_params.remove(generic);
                    }
                    if !f.generics.is_empty() {
                        self.fn_generics.insert(f.name.clone(), f.generics.clone());
                    }
                    self.fn_sigs.insert(f.name.clone(), f.params.iter().map(|p| p.sig()).collect());
                    self.fn_types.insert(f.name.clone(), (params, ret));
                }
                Item::Const(c) => {
                    let ty = match &c.ty {
                        Some(t) => self.ast_type(Some(t)),
                        None => self.fresh(c.span, format!("the type of `{}`", c.name)),
                    };
                    self.scopes.define_with_mutability(&c.name, ty.clone(), false);
                    self.symbols.push(Symbol {
                        name: c.name.clone(),
                        span: c.span,
                        ty,
                        kind: SymbolKind::Variable,
                    });
                    if let Some(declared) = &c.ty {
                        self.validate_decl_type(declared);
                    }
                }
                Item::UiFn(f) => {
                    self.known_fns.insert(f.name.clone());
                    self.ui_fn_names.insert(f.name.clone());
                    // `state name = expr` inside a `ui fn` lives in the
                    // SAME base scope frame a `UiSession` binds it into
                    // at runtime (see tint-runtime/src/ui_session.rs) --
                    // a plain top-level `fn` used as a `click||`/
                    // `hover_in||` handler reads/writes it as an
                    // ordinary variable, with no other declaration of it
                    // anywhere. Without this, checking such a handler's
                    // body would flag every state variable it touches as
                    // unknown (confirmed against `examples/ui_app.tn`,
                    // which does exactly this).
                    for state in &f.state {
                        let ty = self.fresh(state.span, format!("the type of state `{}`", state.name));
                        self.scopes.define_with_mutability(&state.name, ty, true);
                    }
                }
                Item::Struct(s) => {
                    self.symbols.push(Symbol {
                        name: s.name.clone(),
                        span: s.span,
                        ty: Type::Struct(s.name.clone()),
                        kind: SymbolKind::Struct,
                    });
                    self.known_fns.insert(s.name.clone());
                    self.type_names.insert(s.name.clone());
                    self.collect_struct(s);
                }
                Item::ExportStruct(s) => {
                    self.symbols.push(Symbol {
                        name: s.name.clone(),
                        span: s.span,
                        ty: Type::Struct(s.name.clone()),
                        kind: SymbolKind::Struct,
                    });
                    self.known_fns.insert(s.name.clone());
                    self.type_names.insert(s.name.clone());
                    self.collect_struct(s);
                }
                Item::TypeAlias(alias) => self.validate_decl_type(&alias.ty),
                Item::Enum(e) => {
                    self.symbols.push(Symbol {
                        name: e.name.clone(),
                        span: e.span,
                        ty: Type::Enum(e.name.clone()),
                        kind: SymbolKind::Enum,
                    });
                    self.type_names.insert(e.name.clone());
                    self.collect_enum_names(e);
                }
                Item::ExportEnum(e) => {
                    self.symbols.push(Symbol {
                        name: e.name.clone(),
                        span: e.span,
                        ty: Type::Enum(e.name.clone()),
                        kind: SymbolKind::Enum,
                    });
                    self.type_names.insert(e.name.clone());
                    self.collect_enum_names(e);
                }
                Item::GlobalLet(Stmt::Let { pattern, ty, .. }) => {
                    let var = match ty {
                        Some(t) => self.ast_type(Some(t)),
                        None => self.fresh(pattern.span(), "the type of a top-level binding"),
                    };
                    self.global_lets
                        .insert((pattern.span().start.offset, pattern.span().end.offset), var.clone());
                    self.bind_pattern_typed(pattern, var);
                }
                _ => {}
            }
        }

        for item in items {
            if let Item::Impl(block) = item {
                let target = self.ast_type(Some(&block.target));
                if let Type::Struct(name) = target {
                    self.validate_decl_type(&block.target);
                    for method in &block.methods {
                        for param in &method.params {
                            if let Some(ty) = &param.ty {
                                self.validate_decl_type(ty);
                            }
                        }
                        if let Some(ty) = &method.ret_ty {
                            self.validate_decl_type(ty);
                        }
                        let mut params = Vec::with_capacity(method.params.len());
                        for p in method.params.iter().filter(|p| !is_self_pattern(&p.pattern)) {
                            params.push(self.param_type(&format!("{name}.{}", method.name), p));
                        }
                        let ret = match &method.ret_ty {
                            Some(t) => self.ast_type(Some(t)),
                            None => self.fresh(
                                method.span,
                                format!("the return type of `{name}.{}`", method.name),
                            ),
                        };
                        self.unannotated.insert(
                            (name.clone(), method.name.clone()),
                            method
                                .params
                                .iter()
                                .filter(|p| !is_self_pattern(&p.pattern))
                                .map(|p| p.ty.is_none())
                                .collect(),
                        );
                        self.methods
                            .insert((name.clone(), method.name.clone()), (params, ret));
                    }
                }
            }
        }
    }

    fn ast_type(&self, ty: Option<&tint_ast::Type>) -> Type {
        let Some(ty) = ty else { return Type::Unknown };
        match ty {
            tint_ast::Type::Simple(name) => {
                if self.generic_params.contains(name) {
                    return Type::Simple(name.clone());
                }
                match name.as_str() {
                "()" | "unit" => Type::Unit,
                "string" | "str" => Type::String,
                "bool" => Type::Bool,
                "i32" | "i64" | "u8" | "u32" | "u64" | "f32" | "f64" => {
                    Type::Simple(name.clone())
                }
                "number" => Type::Number,
                _ if self.enum_names.contains(name) => Type::Enum(name.clone()),
                    _ => Type::Struct(name.clone()),
                }
            }
            tint_ast::Type::Generic(name, args) if name == "Vec" || name == "Array" => {
                Type::Array(Box::new(self.ast_type(args.first())))
            }
            tint_ast::Type::Generic(name, args) if name == "Option" || name == "Result" => {
                Type::Generic(
                    name.clone(),
                    args.iter().map(|arg| self.ast_type(Some(arg))).collect(),
                )
            }
            tint_ast::Type::Union(types) => {
                Type::Tuple(types.iter().map(|t| self.ast_type(Some(t))).collect())
            }
            tint_ast::Type::Function { params, ret } => Type::Fn(
                Box::new(self.ast_type(Some(ret))),
                params.iter().map(|p| self.ast_type(Some(p))).collect(),
            ),
            tint_ast::Type::Unit => Type::Unit,
            tint_ast::Type::Generic(name, args) => Type::Generic(
                name.clone(),
                args.iter().map(|arg| self.ast_type(Some(arg))).collect(),
            ),
        }
    }

    fn collect_struct(&mut self, s: &tint_ast::StructDecl) {
        self.generic_params.extend(s.generics.iter().cloned());
        let mut fields = HashMap::new();
        for member in &s.members {
            if let tint_ast::StructMember::Field(field) = member {
                match field {
                    tint_ast::StructField::Typed { name, ty, .. }
                    | tint_ast::StructField::TintTyped { name, ty, .. } => {
                        self.validate_decl_type(ty);
                        fields.insert(name.clone(), self.ast_type(Some(ty)));
                    }
                    tint_ast::StructField::TintField { name, span } => {
                        let var = self.fresh(*span, format!("the type of field `{}.{}`", s.name, name));
                        fields.insert(name.clone(), var);
                        self.untyped_fields.insert((s.name.clone(), name.clone()));
                    }
                }
            }
        }
        self.struct_fields.insert(s.name.clone(), fields);
        for generic in &s.generics {
            self.generic_params.remove(generic);
        }
    }

    fn validate_decl_type(&mut self, ty: &tint_ast::Type) {
        match ty {
            tint_ast::Type::Simple(name) => {
                if self.generic_params.contains(name) {
                    return;
                }
                let builtin = matches!(
                    name.as_str(),
                    "unit"
                        | "()"
                        | "string"
                        | "str"
                        | "bool"
                        | "i32"
                        | "i64"
                        | "u8"
                        | "u32"
                        | "u64"
                        | "f32"
                        | "f64"
                        | "number"
                        | "Vec2"
                );
                if !builtin && !self.type_names.contains(name) {
                    self.error(ty.span(), SemanticErrorKind::UnknownIdent(name.clone()));
                }
                if matches!(name.as_str(), "Option" | "Result") {
                    let expected = self.generic_arity.get(name).copied().unwrap_or(0);
                    self.error(
                        ty.span(),
                        SemanticErrorKind::TypeMismatch {
                            expected: format!("{name} with {expected} type argument(s)"),
                            found: format!("{name} with 0 type argument(s)"),
                        },
                    );
                }
            }
            tint_ast::Type::Generic(name, args) => {
                if !matches!(name.as_str(), "Vec" | "Array" | "Map" | "Option" | "Result")
                    && !self.type_names.contains(name)
                {
                    self.error(ty.span(), SemanticErrorKind::UnknownIdent(name.clone()));
                }
                if let Some(expected) = self.generic_arity.get(name) {
                    if *expected != args.len() {
                        self.error(
                            ty.span(),
                            SemanticErrorKind::TypeMismatch {
                                expected: format!("{name} with {expected} type argument(s)"),
                                found: format!("{name} with {} type argument(s)", args.len()),
                            },
                        );
                    }
                }
                for arg in args {
                    self.validate_decl_type(arg);
                }
            }
            tint_ast::Type::Union(types) => {
                for member in types {
                    self.validate_decl_type(member);
                }
            }
            tint_ast::Type::Function { params, ret } => {
                for param in params {
                    self.validate_decl_type(param);
                }
                self.validate_decl_type(ret);
            }
            tint_ast::Type::Unit => {}
        }
    }

    fn collect_enum_names(&mut self, e: &tint_ast::EnumDecl) {
        self.generic_params.extend(e.generics.iter().cloned());
        self.known_fns.insert(e.name.clone());
        for v in &e.variants {
            let (name, fields, names) = match v {
                EnumVariant::Unit(n) => (n, Vec::new(), Vec::new()),
                EnumVariant::Tuple(n, fields) => (
                    n,
                    fields
                        .iter()
                        .map(|ty| {
                            self.validate_decl_type(ty);
                            self.ast_type(Some(ty))
                        })
                        .collect::<Vec<_>>(),
                    vec![None; fields.len()],
                ),
                EnumVariant::Struct(n, fields) => {
                    let mut types = Vec::new();
                    let mut names = Vec::new();
                    for field in fields {
                        match field {
                            tint_ast::StructField::Typed { name, ty, .. }
                            | tint_ast::StructField::TintTyped { name, ty, .. } => {
                                self.validate_decl_type(ty);
                                types.push(self.ast_type(Some(ty)));
                                names.push(Some(name.clone()));
                            }
                            tint_ast::StructField::TintField { name, span } => {
                                types.push(self.fresh(
                                    *span,
                                    format!("the type of field `{}::{}.{}`", e.name, n, name),
                                ));
                                names.push(Some(name.clone()));
                            }
                        }
                    }
                    (n, types, names)
                }
            };
            self.known_fns.insert(name.clone());
            self.variant_fields
                .insert((e.name.clone(), name.clone()), names);
            self.enum_variants
                .insert((e.name.clone(), name.clone()), fields);
        }
        for generic in &e.generics {
            self.generic_params.remove(generic);
        }
    }

    fn visit_item(&mut self, item: &Item) {
        match item {
            Item::Fn(f) | Item::ExportFn(f, _) => self.check_fn_body(f),
            Item::UiFn(f) => self.visit_ui_fn(f),
            // Each method is a plain `FnDecl` -- `self` binds through the
            // ordinary `Pattern::Ident("self", ..)` the parser already
            // produces for it (see `parse_pattern`'s `SelfKw` case), so
            // `check_fn_body` needs no changes to also cover these.
            Item::Impl(block) => {
                for method in &block.methods {
                    self.check_method_body(method, self.ast_type(Some(&block.target)));
                }
            }
            // `GlobalLet`'s pattern was already bound into the persistent
            // base scope in `collect_top_level_names`; the initializer
            // still needs checking, in its own pass so a later global's
            // init can reference an earlier one.
            Item::GlobalLet(Stmt::Let { pattern, init, .. }) => {
                let key = (pattern.span().start.offset, pattern.span().end.offset);
                let expected = self.global_lets.get(&key).cloned();
                let found = self.infer_expr_with(init.expr(), expected.as_ref());
                if let Some(expected) = expected {
                    self.require_compatible(&expected, &found, init.span());
                }
            }
            Item::Const(c) => {
                let expected = self.scopes.lookup(&c.name).unwrap_or(Type::Unknown);
                let found = self.infer_expr_with(&c.init, Some(&expected));
                self.require_compatible(&expected, &found, c.init.span());
            }
            // Not yet covered by this pass, and not oversights: struct/enum
            // declarations themselves (nothing to check about a
            // declaration in isolation without a type checker), and
            // `Item::Mod`/`Item::Use` (module-resolution concerns handled
            // entirely by `tint-cli`'s loader before this checker ever
            // runs -- see that module's doc comment).
            _ => {}
        }
    }

    fn check_fn_body(&mut self, f: &tint_ast::FnDecl) {
        let (params, ret) = self
            .fn_types
            .get(&f.name)
            .cloned()
            .unwrap_or((Vec::new(), Type::Unknown));
        // Defaults are evaluated at the call site with only globals in scope.
        for (param, ty) in f.params.iter().zip(&params) {
            match &param.default {
                Some(tint_ast::DefaultValue::Single(default)) => {
                    let found = self.infer_expr_with(default, Some(ty));
                    self.require_compatible(ty, &found, default.span());
                }
                Some(tint_ast::DefaultValue::Broadcast(default)) => {
                    self.error(default.span(), SemanticErrorKind::Unsupported);
                }
                None => {}
            }
        }
        self.generic_params.extend(f.generics.iter().cloned());
        self.check_body(f, None, ret);
        for generic in &f.generics {
            self.generic_params.remove(generic);
        }
    }

    fn check_method_body(&mut self, f: &tint_ast::FnDecl, receiver: Type) {
        let ret = match &receiver {
            Type::Struct(name) => self.methods.get(&(name.clone(), f.name.clone())).map(|sig| sig.1.clone()),
            _ => None,
        }
        .unwrap_or_else(|| match &f.ret_ty {
            Some(t) => self.ast_type(Some(t)),
            None => Type::Unknown,
        });
        self.check_body(f, Some(receiver), ret);
    }

    fn check_body(&mut self, f: &tint_ast::FnDecl, receiver: Option<Type>, ret: Type) {
        let previous_return = self.current_return.replace(ret.clone());
        self.scopes.push();
        self.bind_params(f, receiver);
        match &f.body {
            FnBody::Block(block) => {
                let found = self.visit_block_in_current_scope(block);
                self.require_compatible(&ret, &found, block.span);
            }
            FnBody::Expr(e) => {
                let found = self.infer_expr_with(e, Some(&ret));
                self.require_compatible(&ret, &found, e.span());
            }
        }
        self.scopes.pop();
        self.current_return = previous_return;
        self.solve_deferred(false);
    }

    // Binds every identifier a pattern introduces into the current scope
    // (reused for `let`, function params, and match-arm patterns).
    // Literal/wildcard sub-patterns bind nothing.
}

/// `self`, `mut self` or `&mut self` as a method parameter.
fn is_self_pattern(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Ident(name, _) => name == "self",
        Pattern::Mut { inner, .. } => is_self_pattern(inner),
        _ => false,
    }
}

impl SemanticChecker {
    /// The declared type of a parameter, or a fresh variable that call sites
    /// and the body will pin down.
    fn param_type(&mut self, owner: &str, param: &tint_ast::Param) -> Type {
        match &param.ty {
            Some(t) => self.ast_type(Some(t)),
            None => {
                let name = match &param.pattern {
                    Pattern::Ident(name, _) => name.clone(),
                    _ => "_".to_string(),
                };
                self.fresh(
                    param.pattern.span(),
                    format!("the type of parameter `{name}` of `{owner}`"),
                )
            }
        }
    }

    /// `state x = init` is visible, as a variable, to every plain `fn` used
    /// as a handler. Learn each state's type before any body is checked
    /// instead of after the owning `ui fn` happens to be visited. Problems in
    /// the initializers are reported later, when the `ui fn` is visited.
    fn type_ui_state(&mut self, items: &[Item]) {
        let errors = self.errors.len();
        let references = self.references.len();
        for item in items {
            if let Item::UiFn(f) = item {
                for state in &f.state {
                    let ty = self.infer_expr(&state.init);
                    if let Some(declared) = self.scopes.lookup(&state.name) {
                        self.unify(&declared, &ty);
                    }
                }
            }
        }
        self.errors.truncate(errors);
        self.references.truncate(references);
    }

    /// Binds a fn/method's parameters with the types of its signature.
    fn bind_params(&mut self, f: &tint_ast::FnDecl, receiver: Option<Type>) {
        let signature: Vec<Type> = match &receiver {
            Some(Type::Struct(name)) => self
                .methods
                .get(&(name.clone(), f.name.clone()))
                .map(|e| e.0.clone())
                .unwrap_or_default(),
            _ => self.fn_types.get(&f.name).map(|e| e.0.clone()).unwrap_or_default(),
        };
        let mut idx = 0;
        for param in &f.params {
            if is_self_pattern(&param.pattern) {
                self.scopes.define("self", receiver.clone().unwrap_or(Type::Unknown));
                continue;
            }
            let ty = match signature.get(idx) {
                Some(ty) => ty.clone(),
                None => self.ast_type(param.ty.as_ref()),
            };
            idx += 1;
            self.bind_pattern_typed(&param.pattern, ty);
        }
    }
}
