use super::*;

mod access;
mod calls;
mod compound;
mod eval;
mod match_values;
mod patterns;
mod stdlib;
mod vars;

impl EvalHost for TintVM {
    fn lookup_var(&mut self, name: &str) -> Option<EvalValue> {
        self.tracked_lookup(name)
    }

    fn tracking_supported(&self) -> bool {
        true
    }

    fn track_begin(&mut self) {
        self.begin_tracking();
    }

    fn track_end(&mut self) -> Vec<(String, Option<EvalValue>)> {
        self.end_tracking()
    }

    fn track_merge(&mut self, reads: &[(String, Option<EvalValue>)]) {
        self.merge_tracking(reads);
    }

    fn reads_unchanged(&mut self, reads: &[(String, Option<EvalValue>)]) -> bool {
        self.reads_still_hold(reads)
    }

    fn resolve_function(&mut self, name: &str) -> Option<EvalValue> {
        let f = (**self.logic_functions.get(name)?).clone();
        let params = f
            .params
            .iter()
            .map(|p| match &p.pattern {
                tint_ast::Pattern::Ident(name, _) => name.clone(),
                tint_ast::Pattern::Typed { pat, .. }
                | tint_ast::Pattern::Mut { inner: pat, .. } => match pat.as_ref() {
                    tint_ast::Pattern::Ident(name, _) => name.clone(),
                    _ => "_".into(),
                },
                _ => "_".into(),
            })
            .collect();
        Some(EvalValue::Function {
            name: f.name,
            params,
            body: match f.body {
                tint_ast::FnBody::Block(block) => tint_evaluator::eval_fn::FnBodyKind::Block(block),
                tint_ast::FnBody::Expr(expr) => tint_evaluator::eval_fn::FnBodyKind::Expr(expr),
            },
            env: self.host_capture_env(),
            async_: f.async_,
        })
    }

    fn load_var(&mut self, name: &str, span: Span) -> EvalValue {
        self.host_load_var(name, span)
    }

    fn define_var(&mut self, name: &str, value: EvalValue) {
        self.host_define_var(name, value);
    }

    fn push_scope(&mut self) {
        self.host_push_scope();
    }

    fn pop_scope(&mut self) {
        self.host_pop_scope();
    }

    fn eval_expr(&mut self, expr: &Expr) -> EvalValue {
        self.host_eval_expr(expr)
    }

    fn eval_block(&mut self, block: &Block) -> Flow {
        self.host_eval_block(block)
    }

    fn assign_to(&mut self, lhs: &Expr, value: EvalValue) -> bool {
        self.host_assign_to(lhs, value)
    }

    fn call_fn(&mut self, name: &str, args: &[EvalValue], span: Span) -> EvalResult<EvalValue> {
        self.host_call_fn(name, args, span)
    }

    fn call_user_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<EvalValue> {
        self.host_call_user_fn(name, args, span)
    }

    fn call_method(
        &mut self,
        receiver: EvalValue,
        method: &str,
        args: &[EvalValue],
        span: Span,
    ) -> EvalResult<(EvalValue, EvalValue)> {
        self.host_call_method(receiver, method, args, span)
    }

    fn set_var(&mut self, name: &str, value: EvalValue) {
        self.host_set_var(name, value);
    }

    fn call_value(&mut self, value: EvalValue, args: &[EvalValue], span: Span) -> EvalValue {
        self.host_call_value(value, args, span)
    }

    fn namespace_lookup(&mut self, namespace: EvalValue, item: &str) -> EvalValue {
        self.host_namespace_lookup(namespace, item)
    }

    fn field_lookup(&mut self, object: EvalValue, field: &str) -> EvalValue {
        self.host_field_lookup(object, field)
    }

    fn index_lookup(&mut self, object: EvalValue, index: EvalValue) -> EvalValue {
        self.host_index_lookup(object, index)
    }

    fn capture_env(&mut self) -> std::rc::Rc<tint_evaluator::env::Env> {
        self.host_capture_env()
    }

    fn eval_block_flow(&mut self, block: &Block) -> Flow {
        self.host_eval_block_flow(block)
    }

    fn match_pattern(&mut self, value: &EvalValue, pattern: &tint_ast::Pattern) -> bool {
        self.host_match_pattern(value, pattern)
    }

    fn bind_pattern(&mut self, pattern: &tint_ast::Pattern, value: &EvalValue) -> bool {
        self.host_bind_pattern(pattern, value)
    }

    fn apply_compound(&mut self, left: &EvalValue, op: &str, right: &EvalValue) -> EvalValue {
        self.host_apply_compound(left, op, right)
    }
}
