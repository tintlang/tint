use super::super::*;

impl TintVM {
    pub(super) fn host_eval_expr(&mut self, expr: &Expr) -> EvalValue {
        tint_evaluator::eval_expr::eval_expr(self, expr)
    }

    pub(super) fn host_eval_block_flow(&mut self, block: &Block) -> Flow {
        TintVM::eval_block_flow(self, block)
    }

    pub(super) fn host_eval_block(&mut self, block: &Block) -> Flow {
        self.host_eval_block_flow(block)
    }
}
