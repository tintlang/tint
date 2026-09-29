use super::super::*;

impl TintVM {
    pub(super) fn host_load_var(&mut self, name: &str, _span: Span) -> EvalValue {
        match self.tracked_lookup(name) {
            Some(v) => v,
            None => EvalValue::Unit,
        }
    }

    pub(super) fn host_define_var(&mut self, name: &str, v: EvalValue) {
        self.scopes.define(name, Self::eval_to_rt(v));
    }

    pub(super) fn host_set_var(&mut self, name: &str, v: EvalValue) {
        self.scopes.set(name, Self::eval_to_rt(v));
    }

    pub(super) fn host_push_scope(&mut self) {
        self.scopes.push();
    }
    pub(super) fn host_pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub(super) fn host_assign_to(&mut self, lhs: &Expr, value: EvalValue) -> bool {
        match lhs {
            // x = value
            Expr::Ident(name, _) => {
                self.host_set_var(name, value);
                true
            }

            // `self` is a distinct AST node from an ordinary identifier.
            Expr::SelfKw(_) => {
                self.host_set_var("self", value);
                true
            }

            // obj.field = value
            Expr::Field { target, field, .. } => {
                // 1) evaluate object
                let mut obj = self.eval_expr(target);

                // 2) mutate in-place
                obj.set_field(field, value.clone());

                // 3) write mutated object back
                if !self.host_assign_to(target, obj) {
                    panic!("Failed to assign back into parent object");
                }

                true
            }

            // arr[i] = value
            Expr::Index { target, index, .. } => {
                let mut arr = self.eval_expr(target);
                let idx = self.eval_expr(index).as_int();

                arr.set_index(idx, value.clone());

                if !self.host_assign_to(target, arr) {
                    panic!("Failed to assign updated array back");
                }

                true
            }

            _ => false,
        }
    }
}

// EXPR & BLOCK
