use std::collections::HashMap;
use std::rc::Rc;

use crate::env::Env;
use crate::errors::{EvalError, EvalResult};
use crate::value::Value;
use crate::{eval_expr, eval_block, eval_fn};
use rune_ast::{Expr, Block, Span, FnDecl};

use rune_ast::Pattern;

use crate::eval_host::{EvalHost, Flow};

/// Runtime host / VM
pub struct HostVM {
    pub env: Env,
    pub functions: HashMap<String, FnDecl>,
    pub modules: HashMap<String, Value>,
}

impl HostVM {
    pub fn new() -> Self {
        Self {
            env: Env::new(),
            functions: HashMap::new(),
            modules: HashMap::new(),
        }
    }
}

impl EvalHost for HostVM {

    // VARIABLES
    fn load_var(&mut self, name: &str, span: Span) -> Value {
        self.env.lookup(name).unwrap_or_else(|| {
            panic!("Undefined variable '{}' at {:?}", name, span)
        })
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

    // EVAL
    fn eval_expr(&mut self, expr: &Expr) -> Value {
        eval_expr::eval_expr(self, expr)
    }

    fn eval_block(&mut self, block: &Block) -> Flow {
        let mut last = Value::Unit;

        for stmt in &block.stmts {
            let result = crate::eval_stmt::eval_stmt(self, stmt);

            match result {
                Flow::Return(v) => return Flow::Return(v),
                Flow::Break => return Flow::Break,
                Flow::Continue => return Flow::Continue,
                Flow::Value(v) => last = v,
            }
        }

        Flow::Value(last)
    }

    // FUNCTION CALLS
    fn call_fn(
        &mut self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> EvalResult<Value> {
        // Builtins
        if let Some(b) = self.modules.get(name) {
            if let Value::HostFunction(func) = b {
                return Ok(func(args.to_vec()));
            }
        }

        // User-defined
        if let Some(f) = self.functions.get(name).cloned() {
            // клонировали — теперь нет иммутабельного займа self
            return Ok(eval_fn::eval_user_fn(self, &f, args, span));
        }

        // Unknown
        Err(EvalError::InvalidOp {
            msg: format!("Unknown function '{}'", name),
            span,
        })
    }

    fn call_user_fn(
        &mut self,
        name: &str,
        args: &[Value],
        span: Span,
    ) -> EvalResult<Value> {
        if let Some(f) = self.functions.get(name).cloned() {
            // f — now owned copy
            Ok(eval_fn::eval_user_fn(self, &f, args, span))
        } else {
            Err(EvalError::InvalidOp {
                msg: format!("Unknown user-fn '{}'", name),
                span,
            })
        }
    }

    // call_value(value(args))
    fn call_value(&mut self, value: Value, args: &[Value], span: Span) -> Value {
        match value {

            // 1) host fn
            Value::HostFunction(func) => func(args.to_vec()),

            // 2) user fn (decl)
            Value::Function { name, params, body, env, async_ } => {
                self.push_scope();

                // capture env
                self.env.extend_from(&env);

                for (param, arg) in params.iter().zip(args.iter()) {
                    self.define_var(param, arg.clone());
                }

                let result = match body {
                    eval_fn::FnBodyKind::Block(b) =>
                        match self.eval_block(&b) {
                            Flow::Value(v) | Flow::Return(v) => v,
                            _ => Value::Unit,
                        },

                    eval_fn::FnBodyKind::Expr(e) =>
                        self.eval_expr(&e),
                };

                self.pop_scope();
                result
            }

            // 3) lambda
            Value::Lambda { params, body, closure } => {
                self.push_scope();
                self.env.extend_from(&closure);

                for (param, arg) in params.iter().zip(args.iter()) {
                    self.define_var(param, arg.clone());
                }

                let v = self.eval_expr(&body);
                self.pop_scope();
                v
            }

            // not callable
            _ => panic!("Value is not callable at {:?}", span),
        }
    }

    fn set_var(&mut self, name: &str, value: Value) {
    self.env.set(name, value);
}

fn eval_block_flow(&mut self, block: &Block) -> Flow {
    crate::eval_block::eval_block_flow(self, block)
}

fn match_pattern(&mut self, value: &Value, pat: &Pattern) -> bool {
    match pat {
        Pattern::Wildcard(_) => true,

        Pattern::Number(s, _) => {
            match value {
                Value::Number(n) => n.to_string() == *s,
                _ => false,
            }
        }

        Pattern::String(s, _) => {
            match value {
                Value::String(v) => v == s,
                _ => false,
            }
        }

        Pattern::Ident(_, _) => true,

        Pattern::Variant { name, args, .. } => {
            match value {
                Value::EnumInstance { variant, args: v_args, .. } => {
                    if variant != name { return false; }
                    if args.len() != v_args.len() { return false; }

                    for (p, v) in args.iter().zip(v_args.iter()) {
                        if !self.match_pattern(v, p) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        }
    }
}

    // namespace: base::item
    fn namespace_lookup(&mut self, ns: Value, item: &str) -> Value {
        match ns {
            Value::Namespace { name: _, items } => {
                items.lookup(item).unwrap_or_else(|| {
                    panic!("Unknown namespace item '{}'", item)
                })
            }
            _ => panic!("Value is not a namespace"),
        }
    }

    // ============================================================
    // obj.field
    // ============================================================
    fn field_lookup(&mut self, obj: Value, field: &str) -> Value {
        match obj {
            Value::StructInstance { name:_, fields } => {
                for (k, v) in fields {
                    if k == field {
                        return v;
                    }
                }
                panic!("No field '{}' found", field);
            }
            _ => panic!("Value is not an object"),
        }
    }

    // arr[i]
    fn index_lookup(&mut self, obj: Value, index: Value) -> Value {
        match (obj, index.as_number()) {
            (Value::List(list), Some(i)) => {
                let idx = i as usize;
                list[idx].clone()
            }
            _ => panic!("Invalid indexing"),
        }
    }

    // For lambda closures: return captured Env
    fn capture_env(&mut self) -> Rc<Env> {
        Rc::new(self.env.clone())
    }
}
