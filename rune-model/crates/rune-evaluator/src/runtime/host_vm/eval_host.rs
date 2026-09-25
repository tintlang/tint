use std::rc::Rc;

use rune_ast::{Block, Expr, Pattern, Span};

use super::{access, calls, patterns, HostVM};
use crate::{
    env::Env,
    errors::EvalResult,
    eval_block, eval_expr,
    eval_host::{EvalHost, Flow},
    value::Value,
};

impl EvalHost for HostVM {
    fn load_var(&mut self, name: &str, span: Span) -> Value {
        self.env
            .lookup(name)
            .unwrap_or_else(|| panic!("Undefined variable '{}' at {:?}", name, span))
    }

    fn define_var(&mut self, name: &str, value: Value) {
        self.env.define(name, value);
    }

    fn push_scope(&mut self) {
        self.env.push();
    }

    fn pop_scope(&mut self) {
        self.env.pop();
    }

    fn eval_expr(&mut self, expr: &Expr) -> Value {
        eval_expr::eval_expr(self, expr)
    }

    fn eval_block(&mut self, block: &Block) -> Flow {
        let mut last = Value::Unit;

        for stmt in &block.stmts {
            match crate::eval_stmt::eval_stmt(self, stmt) {
                Flow::Return(value) => return Flow::Return(value),
                Flow::Break => return Flow::Break,
                Flow::Continue => return Flow::Continue,
                Flow::Value(value) => last = value,
            }
        }

        Flow::Value(last)
    }

    fn assign_to(&mut self, lhs: &Expr, value: Value) -> bool {
        match lhs {
            Expr::Ident(name, _) => {
                self.set_var(name, value);
                true
            }
            Expr::Field { target, field, .. } => {
                let mut object = self.eval_expr(target);
                object.set_field(field, value);
                true
            }
            Expr::Index { target, index, .. } => {
                let mut list = self.eval_expr(target);
                let index = self.eval_expr(index).as_int();
                list.set_index(index, value);
                true
            }
            _ => false,
        }
    }

    fn call_fn(&mut self, name: &str, args: &[Value], span: Span) -> EvalResult<Value> {
        calls::call_fn(self, name, args, span)
    }

    fn call_user_fn(&mut self, name: &str, args: &[Value], span: Span) -> EvalResult<Value> {
        calls::call_user_fn(self, name, args, span)
    }

    fn call_value(&mut self, value: Value, args: &[Value], span: Span) -> Value {
        calls::call_value(self, value, args, span)
    }

    fn set_var(&mut self, name: &str, value: Value) {
        self.env.set(name, value);
    }

    fn eval_block_flow(&mut self, block: &Block) -> Flow {
        eval_block::eval_block_flow(self, block)
    }

    fn bind_pattern(&mut self, pattern: &Pattern, value: &Value) -> bool {
        patterns::bind_pattern(self, pattern, value)
    }

    fn match_pattern(&mut self, value: &Value, pattern: &Pattern) -> bool {
        patterns::match_pattern(self, value, pattern)
    }

    fn namespace_lookup(&mut self, namespace: Value, item: &str) -> Value {
        access::namespace_lookup(namespace, item)
    }

    fn field_lookup(&mut self, object: Value, field: &str) -> Value {
        access::field_lookup(object, field)
    }

    fn index_lookup(&mut self, object: Value, index: Value) -> Value {
        access::index_lookup(object, index)
    }

    fn capture_env(&mut self) -> Rc<Env> {
        Rc::new(self.env.clone())
    }

    fn apply_compound(&mut self, left: &Value, op: &str, right: &Value) -> Value {
        access::apply_compound(left, op, right)
    }
}
