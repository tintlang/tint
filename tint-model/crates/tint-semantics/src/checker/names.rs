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
        for builtin in ["print", "dbg", "sqrt"] {
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
                        self.scopes.define(&state.name, Type::Unit);
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
            // `GlobalLet`'s pattern was already bound into the persistent
            // base scope in `collect_top_level_names`; the initializer
            // still needs checking, in its own pass so a later global's
            // init can reference an earlier one.
            Item::GlobalLet(Stmt::Let { init, .. }) => {
                self.visit_expr(init.expr());
            }
            // Not yet covered by this pass -- see the struct doc comment:
            // `ui fn` bodies, `impl` method bodies, struct/enum
            // declarations themselves (nothing to check about a
            // declaration in isolation without a type checker), modules.
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
