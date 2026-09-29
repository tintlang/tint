impl SemanticChecker {
    pub fn new(ctx: CheckerContext) -> Self {
        SemanticChecker {
            ctx,
            scopes: Scopes::new(),
            known_fns: HashSet::new(),
            ui_tokens: HashSet::new(),
            ui_styles: HashSet::new(),
            ui_variants: HashSet::new(),
            fn_types: HashMap::new(),
            struct_fields: HashMap::new(),
            methods: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_names: HashSet::new(),
            type_names: HashSet::new(),
            generic_arity: HashMap::new(),
            generic_params_by_type: HashMap::new(),
            generic_params: HashSet::new(),
            current_return: None,
            observed_returns: None,
            call_args: HashMap::new(),
            record_calls: false,
            errors: vec![],
            inferred: HashMap::new(),
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
        self.references.clear();
        self.symbols.clear();
        self.known_fns.extend(self.external_context.known_fns.iter().cloned());
        self.collect_top_level_names(&program.items);
        self.type_ui_state(&program.items);
        self.infer_return_types(&program.items);
        for item in &program.items {
            self.visit_item(item);
        }
        self.errors.clone()
    }

    pub fn check_with_model(&mut self, program: &Program) -> (Vec<SemanticError>, SemanticModel) {
        let errors = self.check(program);
        let mut expressions: Vec<_> = self.inferred.values().cloned().collect();
        expressions.sort_by_key(|typed| (typed.span.start.offset, typed.span.end.offset));
        (
            errors,
            SemanticModel {
                expressions,
                symbols: self.symbols.clone(),
                references: self.references.clone(),
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
        self.enum_variants
            .insert(("Option".into(), "Some".into()), vec![Type::Unknown]);
        self.enum_variants
            .insert(("Option".into(), "None".into()), vec![]);
        self.enum_variants
            .insert(("Result".into(), "Ok".into()), vec![Type::Unknown]);
        self.enum_variants
            .insert(("Result".into(), "Err".into()), vec![Type::Unknown]);
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
                    let params = f
                        .params
                        .iter()
                        .map(|p| self.ast_type(p.ty.as_ref()))
                        .collect::<Vec<_>>();
                    let ret = f
                        .ret_ty
                        .as_ref()
                        .map(|t| self.ast_type(Some(t)))
                        .unwrap_or(Type::Unknown);
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
                    self.fn_types.insert(f.name.clone(), (params, ret));
                }
                Item::Const(c) => {
                    let ty = self.ast_type(c.ty.as_ref());
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
                        self.scopes
                            .define_with_mutability(&state.name, Type::Unknown, true);
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
                Item::GlobalLet(Stmt::Let { pattern, .. }) => {
                    self.bind_pattern(pattern);
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
                            params.push(self.ast_type(p.ty.as_ref()));
                        }
                        let ret = method
                            .ret_ty
                            .as_ref()
                            .map(|t| self.ast_type(Some(t)))
                            .unwrap_or(Type::Unknown);
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
                    tint_ast::StructField::TintField { name, .. } => {
                        fields.insert(name.clone(), Type::Unknown);
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
            let (name, fields) = match v {
                EnumVariant::Unit(n) => (n, Vec::new()),
                EnumVariant::Tuple(n, fields) => (
                    n,
                    fields
                        .iter()
                        .map(|ty| {
                            self.validate_decl_type(ty);
                            self.ast_type(Some(ty))
                        })
                        .collect(),
                ),
                EnumVariant::Struct(n, fields) => (
                    n,
                    fields
                        .iter()
                        .map(|field| match field {
                            tint_ast::StructField::Typed { ty, .. }
                            | tint_ast::StructField::TintTyped { ty, .. } => {
                                self.validate_decl_type(ty);
                                self.ast_type(Some(ty))
                            }
                            tint_ast::StructField::TintField { .. } => Type::Unknown,
                        })
                        .collect(),
                ),
            };
            self.known_fns.insert(name.clone());
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
            Item::GlobalLet(Stmt::Let { init, .. }) => {
                self.visit_expr(init.expr());
            }
            Item::Const(c) => {
                let expected = self.ast_type(c.ty.as_ref());
                let found = self.infer_expr(&c.init);
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
        let previous_return = self.current_return.clone();
        self.current_return = f.ret_ty.as_ref().map(|t| self.ast_type(Some(t)));
        self.scopes.push();
        self.bind_params(f, None);
        match &f.body {
            FnBody::Block(block) => {
                let found = self.visit_block_in_current_scope(block);
                if let Some(expected) = self.current_return.clone() {
                    self.require_compatible(&expected, &found, block.span);
                }
            }
            FnBody::Expr(e) => self.visit_expr(e),
        }
        self.scopes.pop();
        self.current_return = previous_return;
    }

    fn check_method_body(&mut self, f: &tint_ast::FnDecl, receiver: Type) {
        let previous_return = self.current_return.clone();
        self.current_return = f.ret_ty.as_ref().map(|t| self.ast_type(Some(t)));
        self.scopes.push();
        self.bind_params(f, Some(receiver));
        match &f.body {
            FnBody::Block(block) => {
                let found = self.visit_block_in_current_scope(block);
                if let Some(expected) = self.current_return.clone() {
                    self.require_compatible(&expected, &found, block.span);
                }
            }
            FnBody::Expr(e) => self.visit_expr(e),
        }
        self.scopes.pop();
        self.current_return = previous_return;
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
    /// `state x = init` is visible, as a variable, to every plain `fn` used
    /// as a handler. Give those variables their real types before any body
    /// is checked, instead of after the owning `ui fn` happens to be visited.
    /// Problems in the initializers are reported later, when the `ui fn` is
    /// visited; this pass only learns the types.
    fn type_ui_state(&mut self, items: &[Item]) {
        let errors = self.errors.len();
        let references = self.references.len();
        for item in items {
            if let Item::UiFn(f) = item {
                for state in &f.state {
                    let ty = self.infer_expr(&state.init);
                    self.scopes.update_type(&state.name, ty);
                }
            }
        }
        self.errors.truncate(errors);
        self.references.truncate(references);
    }

    /// Binds a fn/method's parameters; unannotated ones take the type
    /// inferred from call sites (stored in `fn_types` / `methods`).
    fn bind_params(&mut self, f: &tint_ast::FnDecl, receiver: Option<Type>) {
        let inferred: Vec<Type> = match &receiver {
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
            let ty = match &param.ty {
                Some(t) => self.ast_type(Some(t)),
                None => inferred.get(idx).cloned().unwrap_or(Type::Unknown),
            };
            idx += 1;
            self.bind_pattern_typed(&param.pattern, ty);
        }
    }

    /// Return and unannotated-parameter types of functions/methods, found by
    /// dry-running bodies a few times (diagnostics discarded): call sites
    /// give parameter types, bodies give return types, and each round sees
    /// the previous round's results. Only reliable results are kept: all
    /// call sites must agree, and a body whose value comes from a trailing
    /// `if` / `match` statement stays `Unknown`. If the rounds do not
    /// converge, everything inferred is dropped.
    fn infer_return_types(&mut self, items: &[Item]) {
        let errors = self.errors.len();
        let references = self.references.len();
        let symbols = self.symbols.len();
        let saved_fns = self.fn_types.clone();
        let saved_methods = self.methods.clone();
        let mut converged = false;
        for _ in 0..5 {
            self.call_args.clear();
            self.record_calls = true;
            let mut rets: Vec<(Option<(String, String)>, String, Type)> = Vec::new();
            for item in items {
                match item {
                    Item::Fn(f) | Item::ExportFn(f, _) => {
                        let ret = self.dry_run_return(f, None);
                        if f.ret_ty.is_none() {
                            if let Some(ret) = ret {
                                rets.push((None, f.name.clone(), ret));
                            }
                        }
                    }
                    Item::UiFn(f) => self.visit_ui_fn(f),
                    Item::Impl(block) => {
                        let receiver = self.ast_type(Some(&block.target));
                        let Type::Struct(name) = receiver.clone() else { continue };
                        for m in &block.methods {
                            let ret = self.dry_run_return(m, Some(receiver.clone()));
                            if m.ret_ty.is_none() {
                                if let Some(ret) = ret {
                                    rets.push((Some((name.clone(), m.name.clone())), m.name.clone(), ret));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            self.record_calls = false;
            let mut changed = false;
            for (owner, name, ret) in rets {
                let entry = match owner {
                    None => self.fn_types.get_mut(&name),
                    Some(key) => self.methods.get_mut(&key),
                };
                if let Some(entry) = entry {
                    if entry.1 != ret {
                        entry.1 = ret;
                        changed = true;
                    }
                }
            }
            for item in items {
                match item {
                    Item::Fn(f) | Item::ExportFn(f, _) => {
                        let key = (String::new(), f.name.clone());
                        changed |= self.apply_call_params(f, &key, None);
                    }
                    Item::Impl(block) => {
                        let Type::Struct(name) = self.ast_type(Some(&block.target)) else { continue };
                        for m in &block.methods {
                            let key = (name.clone(), m.name.clone());
                            changed |= self.apply_call_params(m, &key, Some(name.clone()));
                        }
                    }
                    _ => {}
                }
            }
            if !changed {
                converged = true;
                break;
            }
        }
        self.record_calls = false;
        self.call_args.clear();
        if !converged {
            self.fn_types = saved_fns;
            self.methods = saved_methods;
        }
        self.errors.truncate(errors);
        self.references.truncate(references);
        self.symbols.truncate(symbols);
        self.inferred.clear();
    }

    /// Sets unannotated parameter types from the recorded call sites.
    fn apply_call_params(
        &mut self,
        f: &tint_ast::FnDecl,
        key: &(String, String),
        owner: Option<String>,
    ) -> bool {
        let Some(calls) = self.call_args.get(key).cloned() else { return false };
        let untyped: Vec<Option<()>> = f
            .params
            .iter()
            .filter(|p| !is_self_pattern(&p.pattern))
            .map(|p| p.ty.is_none().then_some(()))
            .collect();
        let entry = match &owner {
            None => self.fn_types.get_mut(&f.name),
            Some(o) => self.methods.get_mut(&(o.clone(), f.name.clone())),
        };
        let Some(entry) = entry else { return false };
        let mut changed = false;
        for (i, u) in untyped.iter().enumerate() {
            if u.is_none() || i >= entry.0.len() {
                continue;
            }
            let mut seen: Option<Type> = None;
            let mut ok = true;
            for call in &calls {
                match call.get(i) {
                    Some(t) if *t == Type::Unknown || contains_unknown(t) => {}
                    Some(t) => match &seen {
                        None => seen = Some(t.clone()),
                        Some(s) if s == t => {}
                        Some(_) => ok = false,
                    },
                    None => {}
                }
            }
            let ty = if ok { seen.unwrap_or(Type::Unknown) } else { Type::Unknown };
            if entry.0[i] != ty {
                entry.0[i] = ty;
                changed = true;
            }
        }
        changed
    }

    fn dry_run_return(&mut self, f: &tint_ast::FnDecl, receiver: Option<Type>) -> Option<Type> {
        let previous_return = self.current_return.take();
        let previous_observed = self.observed_returns.replace(Vec::new());
        self.scopes.push();
        self.bind_params(f, receiver.clone());
        let tail = match &f.body {
            FnBody::Block(block) => {
                let found = self.visit_block_in_current_scope(block);
                match block.stmts.last() {
                    Some(Stmt::Expr(_)) => Some(found),
                    Some(Stmt::Return(..)) => None,
                    Some(Stmt::If { .. }) | Some(Stmt::Match { .. }) => None,
                    _ => Some(Type::Unit),
                }
            }
            FnBody::Expr(e) => Some(self.infer_expr(e)),
        };
        self.scopes.pop();
        let returns = self.observed_returns.take().unwrap_or_default();
        self.observed_returns = previous_observed;
        self.current_return = previous_return;
        let mut candidates: Vec<Type> = returns;
        if let Some(t) = tail {
            if t != Type::Unit || candidates.is_empty() {
                candidates.push(t);
            }
        } else if candidates.is_empty() {
            return None;
        }
        let first = candidates[0].clone();
        if first == Type::Unknown || contains_unknown(&first) {
            return None;
        }
        if candidates.iter().any(|c| *c != first) {
            return None;
        }
        Some(first)
    }
}

fn contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Generic(_, args) => args.iter().any(contains_unknown),
        _ => false,
    }
}
