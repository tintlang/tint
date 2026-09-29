impl SemanticChecker {
    /// Signatures of the built-in methods on lists, strings and maps (receiver
    /// excluded). `None` after reporting a method that does not exist.
    fn collection_method_sig(
        &mut self,
        receiver: &Type,
        method: &str,
        nargs: usize,
        span: Span,
    ) -> Option<(Vec<Type>, Type)> {
        let (owner, elem) = match receiver {
            Type::Array(elem) => ("List", (**elem).clone()),
            Type::String => ("String", Type::Unknown),
            Type::Map(value) => ("Map", (**value).clone()),
            _ => return None,
        };
        let string = Type::String;
        let opt = |ty: &Type| Type::Generic("Option".into(), vec![ty.clone()]);
        let predicate = |elem: &Type| Type::Fn(Box::new(Type::Bool), vec![elem.clone()]);
        Some(match (owner, method) {
            (_, "len") => (vec![], Type::Number),
            (_, "is_empty") => (vec![], Type::Bool),

            ("List", "contains") => (vec![elem], Type::Bool),
            ("List", "join") => (vec![string], Type::String),
            ("List", "push") => (vec![elem], Type::Unit),
            ("List", "pop") => (vec![], opt(&elem)),
            ("List", "remove") => (vec![Type::Number], elem),
            ("List", "reverse" | "sort") => (vec![], Type::Array(Box::new(elem))),
            ("List", "slice") => (slice_params(nargs), Type::Array(Box::new(elem))),
            ("List", "find") => (vec![predicate(&elem)], opt(&elem)),
            ("List", "filter") => (vec![predicate(&elem)], Type::Array(Box::new(elem))),
            ("List", "map") => {
                let mapped = self.fresh(span, "the result of `map`");
                (
                    vec![Type::Fn(Box::new(mapped.clone()), vec![elem])],
                    Type::Array(Box::new(mapped)),
                )
            }

            ("String", "trim" | "to_upper" | "to_lower") => (vec![], Type::String),
            ("String", "contains" | "starts_with" | "ends_with") => (vec![string], Type::Bool),
            ("String", "slice") => (slice_params(nargs), Type::String),
            ("String", "split") => (vec![string], Type::Array(Box::new(Type::String))),
            ("String", "replace") => (vec![string.clone(), string], Type::String),

            // Map keys are converted to text at runtime, so any key type is accepted.
            ("Map", "has") => (vec![self.fresh(span, "a map key")], Type::Bool),
            ("Map", "get" | "remove") => (vec![self.fresh(span, "a map key")], opt(&elem)),
            ("Map", "set") => (vec![self.fresh(span, "a map key"), elem], Type::Unit),
            ("Map", "keys") => (vec![], Type::Array(Box::new(Type::String))),
            ("Map", "values") => (vec![], Type::Array(Box::new(elem))),

            _ => {
                self.error(
                    span,
                    SemanticErrorKind::UnknownIdent(format!("{owner}.{method}")),
                );
                return None;
            }
        })
    }
}

/// `slice(start)` or `slice(start, end)`: one `number` per argument given.
fn slice_params(nargs: usize) -> Vec<Type> {
    vec![Type::Number; nargs.clamp(1, 2)]
}
