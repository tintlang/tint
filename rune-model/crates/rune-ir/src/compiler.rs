// ============================================
// rune-ir/src/compiler.rs — AST → SSA IR
// ============================================

use crate::ir::*;
use crate::builder::IrBuilder;

use rune_ast::{Program, Item, Expr, Stmt, FnBody};

pub struct SsaCompiler {
    builder: IrBuilder,
}

impl SsaCompiler {
    pub fn new() -> Self {
        Self {
            builder: IrBuilder::new(),
        }
    }

    pub fn compile_program(&mut self, prog: &Program) -> ProgramIR {
        let mut out = ProgramIR::new();

        for item in &prog.items {
            if let Item::Fn(f) = item {
                out.functions.push(self.compile_fn(f));
            }
        }

        out
    }

    fn compile_fn(&mut self, f: &rune_ast::FnDecl) -> FunctionIR {
        let mut block = self.builder.new_block();
        let mut locals = std::collections::HashMap::new();

        match &f.body {
            FnBody::Block(b) => {
                for stmt in &b.stmts {
                    self.lower_stmt(stmt, &mut block, &mut locals);
                }
            }

            FnBody::Expr(expr) => {
                let v = self.lower_expr(expr, &mut block, &mut locals);
                self.builder.emit_return(&mut block, v);
            }
        }

        FunctionIR {
            name: f.name.clone(),
            blocks: vec![block],
            locals,
        }
    }

    fn lower_stmt(
        &mut self,
        s: &Stmt,
        block: &mut Block,
        locals: &mut std::collections::HashMap<String, ValueId>,
    ) {
        match s {
            Stmt::Let { name, expr, .. } => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_store(block, name.clone(), v);
                locals.insert(name.clone(), v);
            }

            Stmt::Assign { name, expr, .. } => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_store(block, name.clone(), v);
                locals.insert(name.clone(), v);
            }

            Stmt::Return(expr, _) => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_return(block, v);
            }

            Stmt::Expr(expr) => {
                self.lower_expr(expr, block, locals);
            }

            _ => {
                // TODO: remaining statements
                println!("WARNING: stmt not lowered in SSA: {:?}", s);
            }
        }
    }

       fn lower_expr(
        &mut self,
        e: &Expr,
        block: &mut Block,
        locals: &mut std::collections::HashMap<String, ValueId>,
    ) -> ValueId {
        match e {
            Expr::Number(n, _) => {
                let v = Value::Number(n.parse().unwrap());
                self.builder.emit_const(block, v)
            }

            Expr::String(s, _) => {
                self.builder.emit_const(block, Value::String(s.clone()))
            }

            Expr::Ident(name, _) => {
                if let Some(&id) = locals.get(name) {
                    id
                } else {
                    self.builder.emit_load(block, name.clone())
                }
            }

            Expr::Binary { left, op, right, .. } => {
                let lv = self.lower_expr(left, block, locals);
                let rv = self.lower_expr(right, block, locals);
                self.builder.emit_binary(block, op.clone(), lv, rv)
            }

            Expr::Unary { op, expr, .. } => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_unary(block, op.clone(), v)
            }

            Expr::Paren(expr, _) => {
                self.lower_expr(expr, block, locals)
            }

            // -----------------------------------------------
            // CALL: foo(a, b)
            // -----------------------------------------------
            Expr::Call { target, args, .. } => {
                let fn_val = self.lower_expr(target, block, locals);
                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.lower_expr(a, block, locals));
                }
                self.builder.emit_call(block, fn_val, arg_vals)
            }

            // -----------------------------------------------
            // FIELD: obj.field
            // -----------------------------------------------
            Expr::Field { target, field, .. } => {
                let base = self.lower_expr(target, block, locals);
                self.builder.emit_field_access(block, base, field.clone())
            }

            // -----------------------------------------------
            // NAMESPACE: A::B
            // -----------------------------------------------
            Expr::Namespace { base, item, .. } => {
                let mod_val = self.lower_expr(base, block, locals);
                self.builder.emit_namespace_access(block, mod_val, item.clone())
            }

            // -----------------------------------------------
            // INDEX: arr[i]
            // -----------------------------------------------
            Expr::Index { target, index, .. } => {
                let arr = self.lower_expr(target, block, locals);
                let idx = self.lower_expr(index, block, locals);
                self.builder.emit_index(block, arr, idx)
            }

            // -----------------------------------------------
            // LAMBDA → пока как TODO
            // -----------------------------------------------
            Expr::Lambda { .. } => {
                let id = self.builder.emit_const(block, Value::Unit);
                println!("WARNING: Lambda lowering not implemented");
                id
            }

            // -----------------------------------------------
            // STRUCT INIT { .. }
            // -----------------------------------------------
            Expr::StructInit { name, fields, .. } => {
                let mut field_vals = Vec::new();
                for (fname, fexpr) in fields {
                    let fv = self.lower_expr(fexpr, block, locals);
                    field_vals.push((fname.clone(), fv));
                }
                self.builder.emit_struct_init(block, name.clone(), field_vals)
            }
        }
    }
}