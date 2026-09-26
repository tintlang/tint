impl SemanticChecker {
    fn bind_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ident(name, span) => {
                if !self.scopes.define(name, Type::Unit) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(name.clone()));
                }
            }
            Pattern::Tuple(items, _) => {
                for p in items {
                    self.bind_pattern(p);
                }
            }
            Pattern::Struct { fields, .. }
            | Pattern::Map { fields, .. }
            | Pattern::Group { fields, .. } => {
                for field in fields {
                    self.bind_pattern_field(field);
                }
            }
            Pattern::Variant { args, .. } => {
                for p in args {
                    self.bind_pattern(p);
                }
            }
            Pattern::Typed { pat, .. } | Pattern::Mut { inner: pat, .. } => self.bind_pattern(pat),
            Pattern::Number(_, _) | Pattern::String(_, _) | Pattern::Wildcard(_) => {}
        }
    }

    fn bind_pattern_field(&mut self, field: &PatternField) {
        match field {
            PatternField::Shorthand { field, span } => {
                if !self.scopes.define(field, Type::Unit) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(field.clone()));
                }
            }
            PatternField::Assign { pat, .. } => self.bind_pattern(pat),
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
    fn visit_block_in_current_scope(&mut self, block: &Block) {
        self.ctx.mode = Mode::Logic;
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
    }
}
