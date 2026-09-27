use crate::builder::IrBuilder;
use crate::ir::*;

use tint_ast::{FnBody, Item, Program};

mod expressions;
mod statements;

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
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    out.functions.push(self.compile_fn(f));
                }
                _ => {}
            }
        }

        out
    }

    fn compile_fn(&mut self, f: &tint_ast::FnDecl) -> FunctionIR {
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
            params: f.params.iter().map(|p| p.pattern.clone()).collect(),
        }
    }
}
