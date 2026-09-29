// Type inference core: inference variables, unification, constraints that
// wait for a receiver type (`Deferred`) and the final "zonk" that turns every
// recorded type into a variable-free one.
impl SemanticChecker {
    fn fresh(&mut self, span: Span, what: impl Into<String>) -> Type {
        self.new_var(span, what.into(), false)
    }

    /// A variable that defaults to `unit` when nothing constrains it: the
    /// payload of `None {}`, the element of `[]`, a diverging branch.
    fn fresh_hole(&mut self, span: Span, what: impl Into<String>) -> Type {
        self.new_var(span, what.into(), true)
    }

    fn new_var(&mut self, span: Span, what: String, default_unit: bool) -> Type {
        let id = self.subst.len() as u32;
        self.subst.push(None);
        self.var_info.push(VarInfo { span, what, default_unit });
        Type::Var(id)
    }

    /// Follows variable bindings until the head constructor is known.
    fn shallow(&self, ty: &Type) -> Type {
        let mut current = ty.clone();
        while let Type::Var(v) = current {
            match &self.subst[v as usize] {
                Some(next) => current = next.clone(),
                None => return Type::Var(v),
            }
        }
        current
    }

    /// The last variable of a chain whose binding is not itself a variable.
    fn terminal_var(&self, ty: &Type) -> Option<u32> {
        let Type::Var(mut v) = *ty else { return None };
        loop {
            match &self.subst[v as usize] {
                Some(Type::Var(next)) => v = *next,
                Some(_) => return Some(v),
                None => return None,
            }
        }
    }

    /// Replaces every solved variable, deeply. Unsolved ones stay `Var`.
    fn resolve(&self, ty: &Type) -> Type {
        match self.shallow(ty) {
            Type::Array(t) => Type::Array(Box::new(self.resolve(&t))),
            Type::Map(t) => Type::Map(Box::new(self.resolve(&t))),
            Type::Tensor(t) => Type::Tensor(Box::new(self.resolve(&t))),
            Type::Tuple(items) => Type::Tuple(items.iter().map(|t| self.resolve(t)).collect()),
            Type::Generic(name, args) => {
                Type::Generic(name, args.iter().map(|t| self.resolve(t)).collect())
            }
            Type::Fn(ret, params) => Type::Fn(
                Box::new(self.resolve(&ret)),
                params.iter().map(|t| self.resolve(t)).collect(),
            ),
            other => other,
        }
    }

    fn occurs(&self, var: u32, ty: &Type) -> bool {
        match self.shallow(ty) {
            Type::Var(v) => v == var,
            Type::Array(t) | Type::Map(t) | Type::Tensor(t) => self.occurs(var, &t),
            Type::Tuple(items) => items.iter().any(|t| self.occurs(var, t)),
            Type::Generic(_, args) => args.iter().any(|t| self.occurs(var, t)),
            Type::Fn(ret, params) => {
                self.occurs(var, &ret) || params.iter().any(|t| self.occurs(var, t))
            }
            _ => false,
        }
    }

    /// Makes `expected` and `found` the same type, binding variables on the
    /// way. `Number` (an unsuffixed literal) matches every sized numeric type;
    /// a variable that so far only knows it is a `Number` is refined to the
    /// sized type. Returns false when the two cannot be the same.
    fn unify(&mut self, expected: &Type, found: &Type) -> bool {
        let a = self.shallow(expected);
        let b = self.shallow(found);
        match (&a, &b) {
            (Type::Var(x), Type::Var(y)) if x == y => true,
            (Type::Var(x), _) => {
                if matches!(b, Type::Unknown) {
                    return true;
                }
                if self.occurs(*x, &b) {
                    return false;
                }
                self.subst[*x as usize] = Some(b);
                true
            }
            (_, Type::Var(y)) => {
                if matches!(a, Type::Unknown) {
                    return true;
                }
                if self.occurs(*y, &a) {
                    return false;
                }
                self.subst[*y as usize] = Some(a);
                true
            }
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            (Type::Number, sized) | (sized, Type::Number) if sized.is_sized_numeric() => {
                // Refine `var := Number` to the sized type the program insists on.
                for side in [expected, found] {
                    if matches!(self.shallow(side), Type::Number) {
                        if let Some(v) = self.terminal_var(side) {
                            self.subst[v as usize] = Some(sized.clone());
                        }
                    }
                }
                true
            }
            (Type::Simple(x), Type::Simple(y)) => x == y,
            (Type::Array(x), Type::Array(y))
            | (Type::Map(x), Type::Map(y))
            | (Type::Tensor(x), Type::Tensor(y)) => self.unify(x, y),
            (Type::Tuple(x), Type::Tuple(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(l, r)| self.unify(l, r))
            }
            (Type::Generic(n, x), Type::Generic(m, y)) => {
                n == m && x.len() == y.len() && x.iter().zip(y).all(|(l, r)| self.unify(l, r))
            }
            (Type::Fn(r1, p1), Type::Fn(r2, p2)) => {
                p1.len() == p2.len()
                    && p1.iter().zip(p2).all(|(l, r)| self.unify(l, r))
                    && self.unify(r1, r2)
            }
            _ => a == b,
        }
    }

    fn require_compatible(&mut self, expected: &Type, found: &Type, span: Span) {
        if !self.unify(expected, found) {
            let (expected, found) = (self.resolve(expected), self.resolve(found));
            self.error(
                span,
                SemanticErrorKind::TypeMismatch {
                    expected: type_name(&expected),
                    found: type_name(&found),
                },
            );
        }
    }

    /// Runs every waiting constraint whose receiver is known by now, until no
    /// more progress is made. With `last_call` set, arithmetic on operands that
    /// are still open falls back to `number`.
    fn solve_deferred(&mut self, last_call: bool) {
        loop {
            let pending = std::mem::take(&mut self.deferred);
            let before = pending.len();
            let mut keep = Vec::new();
            for item in pending {
                if !self.try_solve(&item, last_call) {
                    keep.push(item);
                }
            }
            let added = std::mem::take(&mut self.deferred);
            let progressed = keep.len() < before || !added.is_empty();
            keep.extend(added);
            self.deferred = keep;
            if !progressed {
                break;
            }
        }
    }

    fn try_solve(&mut self, item: &Deferred, last_call: bool) -> bool {
        match item {
            Deferred::Method { recv, method, args, ret, span } => {
                match self.method_sig(recv, method, args.len(), *span) {
                    MethodSig::Pending => false,
                    MethodSig::Known(params, result) => {
                        self.check_method_args(recv, method, &params, args, *span);
                        self.require_compatible(ret, &result, *span);
                        true
                    }
                    MethodSig::Failed => true,
                    MethodSig::NotAMethod => {
                        let callee = self.field_of(recv, method, *span).unwrap_or(Type::Unknown);
                        let result = self.call_type(&callee, args, *span);
                        self.require_compatible(ret, &result, *span);
                        true
                    }
                }
            }
            Deferred::Field { recv, field, ret, span } => match self.field_of(recv, field, *span) {
                Some(found) => {
                    self.require_compatible(ret, &found, *span);
                    true
                }
                None => false,
            },
            Deferred::Index { recv, index, ret, span } => match self.index_of(recv, index, *span) {
                Some(found) => {
                    self.require_compatible(ret, &found, *span);
                    true
                }
                None => false,
            },
            Deferred::TupleIndex { recv, index, ret, span } => {
                match self.tuple_index_of(recv, *index, *span) {
                    Some(found) => {
                        self.require_compatible(ret, &found, *span);
                        true
                    }
                    None => false,
                }
            }
            Deferred::Try { recv, ret, span } => match self.try_of(recv, *span) {
                Some(found) => {
                    self.require_compatible(ret, &found, *span);
                    true
                }
                None => false,
            },
            Deferred::Arith { op, left, right, ret, spans } => {
                match self.arith_type(op, left, right, *spans, last_call) {
                    Some(found) => {
                        self.require_compatible(ret, &found, spans.0);
                        true
                    }
                    None => false,
                }
            }
        }
    }

    /// Ends inference: solves what can be solved, defaults the holes, and
    /// resolves every type recorded so far. Anything still open becomes
    /// `Unknown`, or a `CannotInfer` error in strict mode.
    fn finalize_types(&mut self) {
        self.solve_deferred(false);
        // Only now may holes default: `None {}` matched but never returned, ...
        for v in 0..self.subst.len() {
            if self.subst[v].is_none() && self.var_info[v].default_unit {
                self.subst[v] = Some(Type::Unit);
            }
        }
        self.solve_deferred(true);
        for v in 0..self.subst.len() {
            if self.subst[v].is_none() && self.var_info[v].default_unit {
                self.subst[v] = Some(Type::Unit);
            }
        }
        let inferred: Vec<_> = self.inferred.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (key, typed) in inferred {
            let ty = self.resolve(&typed.ty);
            self.inferred.get_mut(&key).unwrap().ty = ty;
        }
        let by_ptr: Vec<_> = self.inferred_by_ptr.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (key, ty) in by_ptr {
            let ty = self.resolve(&ty);
            self.inferred_by_ptr.insert(key, ty);
        }
        let bindings: Vec<_> = self.binding_types.iter().map(|(k, v)| (*k, v.clone())).collect();
        for (key, ty) in bindings {
            let ty = self.resolve(&ty);
            self.binding_types.insert(key, ty);
        }
        for symbol in &mut self.symbols {
            symbol.ty = resolve_with(&self.subst, &symbol.ty);
        }
        let mut poly: Vec<_> = self.polymorphic.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        poly.sort();
        for (key, indices) in poly {
            for index in indices {
                if self.ctx.strict {
                    let label = if key.0.is_empty() { key.1.clone() } else { format!("{}.{}", key.0, key.1) };
                    self.error(
                        Span::dummy(),
                        SemanticErrorKind::CannotInfer(format!(
                            "parameter {} of `{label}`: it is called with different types, annotate it",
                            index + 1
                        )),
                    );
                }
                let table = if key.0.is_empty() { self.fn_types.get_mut(&key.1) } else { self.methods.get_mut(&key) };
                if let Some(entry) = table {
                    if let Some(slot) = entry.0.get_mut(index) {
                        *slot = Type::Unknown;
                    }
                }
            }
        }
        if self.ctx.strict {
            self.report_uninferred();
        }
        // What is still open is genuinely unknown.
        let subst = self.subst.clone();
        let close = |ty: &Type| close_open_vars(&resolve_with(&subst, ty));
        for typed in self.inferred.values_mut() {
            typed.ty = close(&typed.ty);
        }
        for ty in self.inferred_by_ptr.values_mut() {
            *ty = close(ty);
        }
        for ty in self.binding_types.values_mut() {
            *ty = close(ty);
        }
        for symbol in &mut self.symbols {
            symbol.ty = close(&symbol.ty);
        }
        for (params, ret) in self.fn_types.values_mut().chain(self.methods.values_mut()) {
            for ty in params.iter_mut() {
                *ty = close(ty);
            }
            *ret = close(ret);
        }
        for scope in &mut self.scopes.scopes {
            for ty in scope.values_mut() {
                *ty = close(ty);
            }
        }
        for fields in self.struct_fields.values_mut() {
            for ty in fields.values_mut() {
                *ty = close(ty);
            }
        }
        for payload in self.enum_variants.values_mut() {
            for ty in payload.iter_mut() {
                *ty = close(ty);
            }
        }
    }

    /// Strict mode: one error per variable nothing could determine, at the
    /// place that introduced it, plus one per expression left `Unknown`.
    fn report_uninferred(&mut self) {
        let mut roots: Vec<u32> = Vec::new();
        let collect = |ty: &Type, roots: &mut Vec<u32>| {
            fn walk(ty: &Type, out: &mut Vec<u32>) {
                match ty {
                    Type::Var(v) => {
                        if !out.contains(v) {
                            out.push(*v)
                        }
                    }
                    Type::Array(t) | Type::Map(t) | Type::Tensor(t) => walk(t, out),
                    Type::Tuple(items) => items.iter().for_each(|t| walk(t, out)),
                    Type::Generic(_, args) => args.iter().for_each(|t| walk(t, out)),
                    Type::Fn(ret, params) => {
                        walk(ret, out);
                        params.iter().for_each(|t| walk(t, out))
                    }
                    _ => {}
                }
            }
            walk(ty, roots)
        };
        let mut keys: Vec<_> = self.inferred.keys().copied().collect();
        keys.sort();
        let mut unknown_spans = Vec::new();
        for key in keys {
            let typed = &self.inferred[&key];
            collect(&typed.ty, &mut roots);
            if matches!(typed.ty, Type::Unknown) {
                unknown_spans.push(typed.span);
            }
        }
        let mut reported: Vec<Span> = Vec::new();
        for v in roots {
            let info = self.var_info[v as usize].clone();
            if reported.contains(&info.span) && info.span != Span::dummy() {
                continue;
            }
            reported.push(info.span);
            self.error(info.span, SemanticErrorKind::CannotInfer(info.what));
        }
        for span in unknown_spans {
            if !reported.contains(&span) {
                self.error(span, SemanticErrorKind::CannotInfer("this expression".into()));
            }
        }
    }
}

fn resolve_with(subst: &[Option<Type>], ty: &Type) -> Type {
    match ty {
        Type::Var(v) => match &subst[*v as usize] {
            Some(next) => resolve_with(subst, next),
            None => Type::Var(*v),
        },
        Type::Array(t) => Type::Array(Box::new(resolve_with(subst, t))),
        Type::Map(t) => Type::Map(Box::new(resolve_with(subst, t))),
        Type::Tensor(t) => Type::Tensor(Box::new(resolve_with(subst, t))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(|t| resolve_with(subst, t)).collect()),
        Type::Generic(name, args) => {
            Type::Generic(name.clone(), args.iter().map(|t| resolve_with(subst, t)).collect())
        }
        Type::Fn(ret, params) => Type::Fn(
            Box::new(resolve_with(subst, ret)),
            params.iter().map(|t| resolve_with(subst, t)).collect(),
        ),
        other => other.clone(),
    }
}

fn close_open_vars(ty: &Type) -> Type {
    match ty {
        Type::Var(_) => Type::Unknown,
        Type::Array(t) => Type::Array(Box::new(close_open_vars(t))),
        Type::Map(t) => Type::Map(Box::new(close_open_vars(t))),
        Type::Tensor(t) => Type::Tensor(Box::new(close_open_vars(t))),
        Type::Tuple(items) => Type::Tuple(items.iter().map(close_open_vars).collect()),
        Type::Generic(name, args) => {
            Type::Generic(name.clone(), args.iter().map(close_open_vars).collect())
        }
        Type::Fn(ret, params) => Type::Fn(
            Box::new(close_open_vars(ret)),
            params.iter().map(close_open_vars).collect(),
        ),
        other => other.clone(),
    }
}
