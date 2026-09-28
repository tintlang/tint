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
            current_return: None,
            errors: vec![],
        }
    }

    pub fn check(&mut self, program: &Program) -> Vec<SemanticError> {
        self.collect_top_level_names(&program.items);
        for item in &program.items {
            self.visit_item(item);
        }
        self.errors.clone()
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
            "print", "log", "dbg", "debug", "error", "sqrt", "vec2", "clamp", "min", "max", "abs",
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
        for item in items {
            match item {
                Item::Struct(s) | Item::ExportStruct(s) => {
                    self.type_names.insert(s.name.clone());
                }
                Item::Enum(e) | Item::ExportEnum(e) => {
                    self.type_names.insert(e.name.clone());
                    self.enum_names.insert(e.name.clone());
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
                    let params = f
                        .params
                        .iter()
                        .map(|p| self.ast_type(p.ty.as_ref()))
                        .collect();
                    let ret = f
                        .ret_ty
                        .as_ref()
                        .map(|t| self.ast_type(Some(t)))
                        .unwrap_or(Type::Unknown);
                    self.fn_types.insert(f.name.clone(), (params, ret));
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
                    self.known_fns.insert(s.name.clone());
                    self.type_names.insert(s.name.clone());
                    self.collect_struct(s);
                }
                Item::ExportStruct(s) => {
                    self.known_fns.insert(s.name.clone());
                    self.type_names.insert(s.name.clone());
                    self.collect_struct(s);
                }
                Item::TypeAlias(alias) => self.validate_decl_type(&alias.ty),
                Item::Enum(e) => {
                    self.type_names.insert(e.name.clone());
                    self.collect_enum_names(e);
                }
                Item::ExportEnum(e) => {
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
                        for p in method.params.iter().filter(
                            |p| !matches!(&p.pattern, Pattern::Ident(name, _) if name == "self"),
                        ) {
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
            tint_ast::Type::Simple(name) => match name.as_str() {
                "()" | "unit" => Type::Unit,
                "string" | "str" => Type::String,
                "bool" => Type::Bool,
                "i32" | "i64" | "u32" | "u64" | "f32" | "f64" | "number" => Type::Number,
                _ if self.enum_names.contains(name) => Type::Enum(name.clone()),
                _ => Type::Struct(name.clone()),
            },
            tint_ast::Type::Generic(name, args) if name == "Vec" || name == "Array" => {
                Type::Array(Box::new(self.ast_type(args.first())))
            }
            tint_ast::Type::Union(types) => {
                Type::Tuple(types.iter().map(|t| self.ast_type(Some(t))).collect())
            }
            tint_ast::Type::Unit => Type::Unit,
            tint_ast::Type::Generic(name, _) => Type::Struct(name.clone()),
        }
    }

    fn collect_struct(&mut self, s: &tint_ast::StructDecl) {
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
    }

    fn validate_decl_type(&mut self, ty: &tint_ast::Type) {
        match ty {
            tint_ast::Type::Simple(name) => {
                let builtin = matches!(
                    name.as_str(),
                    "unit"
                        | "()"
                        | "string"
                        | "str"
                        | "bool"
                        | "i32"
                        | "i64"
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
                if !matches!(name.as_str(), "Vec" | "Array" | "Map")
                    && !self.type_names.contains(name)
                {
                    self.error(ty.span(), SemanticErrorKind::UnknownIdent(name.clone()));
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
            tint_ast::Type::Unit => {}
        }
    }

    fn collect_enum_names(&mut self, e: &tint_ast::EnumDecl) {
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
        for param in &f.params {
            self.bind_pattern_typed(&param.pattern, self.ast_type(param.ty.as_ref()));
        }
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
        for param in &f.params {
            if matches!(&param.pattern, Pattern::Ident(name, _) if name == "self") {
                self.scopes.define("self", receiver.clone());
            } else {
                self.bind_pattern_typed(&param.pattern, self.ast_type(param.ty.as_ref()));
            }
        }
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
