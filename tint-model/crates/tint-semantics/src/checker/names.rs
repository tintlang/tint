impl SemanticChecker {
    pub fn new(ctx: CheckerContext) -> Self {
        SemanticChecker {
            ctx,
            scopes: Scopes::new(),
            known_fns: HashSet::new(),
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
            .define_with_mutability("viewport_width", Type::Simple("f64".into()), true);
        // UiSession also provides a default `theme` value for UI trees that
        // use imported theme-switch helpers without declaring `state theme`
        // themselves (Pong is one such consumer).
        self.scopes
            .define_with_mutability("theme", Type::Simple("string".into()), true);

        for builtin in [
            "print", "log", "dbg", "debug", "error", "sqrt", "vec2", "clamp", "min", "max",
            "abs", "sign",
        ] {
            self.known_fns.insert(builtin.to_string());
        }
        for item in items {
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    self.known_fns.insert(f.name.clone());
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
                            .define_with_mutability(&state.name, Type::Unit, true);
                    }
                }
                Item::Struct(s) => {
                    self.known_fns.insert(s.name.clone());
                }
                Item::ExportStruct(s) => {
                    self.known_fns.insert(s.name.clone());
                }
                Item::Enum(e) => self.collect_enum_names(e),
                Item::ExportEnum(e) => self.collect_enum_names(e),
                Item::GlobalLet(Stmt::Let { pattern, .. }) => {
                    self.bind_pattern(pattern);
                }
                _ => {}
            }
        }
    }

    fn collect_enum_names(&mut self, e: &tint_ast::EnumDecl) {
        self.known_fns.insert(e.name.clone());
        for v in &e.variants {
            let name = match v {
                EnumVariant::Unit(n) => n,
                EnumVariant::Tuple(n, _) => n,
                EnumVariant::Struct(n, _) => n,
            };
            self.known_fns.insert(name.clone());
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
                    self.check_fn_body(method);
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
        self.scopes.push();
        for param in &f.params {
            self.bind_pattern(&param.pattern);
        }
        match &f.body {
            FnBody::Block(block) => self.visit_block_in_current_scope(block),
            FnBody::Expr(e) => self.visit_expr(e),
        }
        self.scopes.pop();
    }

    // Binds every identifier a pattern introduces into the current scope
    // (reused for `let`, function params, and match-arm patterns).
    // Literal/wildcard sub-patterns bind nothing.
}
