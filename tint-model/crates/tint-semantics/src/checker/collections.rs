impl SemanticChecker {
    /// Types for the built-in methods on lists, strings and maps. `None`
    /// when `receiver` is none of those, so the caller keeps its own lookup.
    fn infer_collection_method(
        &mut self,
        receiver: &Type,
        method: &str,
        args: &[Type],
        span: Span,
    ) -> Option<Type> {
        let (owner, elem) = match receiver {
            Type::Array(elem) => ("List", (**elem).clone()),
            Type::String => ("String", Type::Unknown),
            Type::Map(value) => ("Map", (**value).clone()),
            _ => return None,
        };
        let string = Type::String;
        let opt = |ty: &Type| Type::Generic("Option".into(), vec![ty.clone()]);
        // (expected argument types, result type)
        let (params, ret): (Vec<Type>, Type) = match (owner, method) {
            (_, "len") => (vec![], Type::Number),
            (_, "is_empty") => (vec![], Type::Bool),

            ("List", "contains") => (vec![elem], Type::Bool),
            ("List", "join") => (vec![string], Type::String),
            ("List", "push") => (vec![elem], Type::Unit),
            ("List", "pop") => (vec![], opt(&elem)),
            ("List", "remove") => (vec![Type::Number], elem),
            ("List", "reverse" | "sort") => (vec![], Type::Array(Box::new(elem))),
            ("List", "slice") => (slice_params(args), Type::Array(Box::new(elem))),
            ("List", "find") => (vec![Type::Unknown], opt(&elem)),
            ("List", "filter") => (vec![Type::Unknown], Type::Array(Box::new(elem))),
            ("List", "map") => {
                let mapped = match args.first() {
                    Some(Type::Fn(ret, _)) => (**ret).clone(),
                    _ => Type::Unknown,
                };
                (vec![Type::Unknown], Type::Array(Box::new(mapped)))
            }

            ("String", "trim" | "to_upper" | "to_lower") => (vec![], Type::String),
            ("String", "contains" | "starts_with" | "ends_with") => (vec![string], Type::Bool),
            ("String", "slice") => (slice_params(args), Type::String),
            ("String", "split") => (vec![string], Type::Array(Box::new(Type::String))),
            ("String", "replace") => (vec![string.clone(), string], Type::String),

            // Map keys are converted to text at runtime, so any key type is accepted.
            ("Map", "has") => (vec![Type::Unknown], Type::Bool),
            ("Map", "get" | "remove") => (vec![Type::Unknown], opt(&elem)),
            ("Map", "set") => (vec![Type::Unknown, elem], Type::Unit),
            ("Map", "keys") => (vec![], Type::Array(Box::new(Type::String))),
            ("Map", "values") => (vec![], Type::Array(Box::new(elem))),

            _ => {
                self.error(
                    span,
                    SemanticErrorKind::UnknownIdent(format!("{owner}.{method}")),
                );
                return Some(Type::Unknown);
            }
        };
        self.check_args(&params, args, span);
        Some(ret)
    }
}

/// `slice(start)` or `slice(start, end)`: one `number` per argument given.
fn slice_params(args: &[Type]) -> Vec<Type> {
    vec![Type::Number; args.len().clamp(1, 2)]
}
