use super::*;

mod access;
mod calls;
mod compound;
mod eval;
mod match_values;
mod patterns;
mod vars;

impl EvalHost for TintVM {
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
