pub mod stmt;
pub mod expr;
pub mod call;

use crate::vm::VM;
use rune_ast::*;

impl VM {
    pub fn eval_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.eval_stmt(stmt);
        }
    }
}
