use super::*;

impl TintVM {
    pub(super) fn call_ui_fn(
        &mut self,
        ui: &UiFnDecl,
        args: &[EvalValue],
        _span: Span,
    ) -> EvalResult<EvalValue> {
        // 1) Parameters
        for (param, arg) in ui.params.iter().zip(args.iter()) {
            bind_pattern(self, &param.pattern, arg);
        }

        // 2) Nothing happens here yet.
        // Later:
        // - interpolation
        // - UiBuilder
        // - UiTree
        // - reconciliation
        // - layout
        // - WebGPU render
        self.pop_scope();

        Ok(EvalValue::Unit)
    }

    /// Mounts `name` (a `ui fn`) fresh and returns its rendered tree --
    /// the top-level nodes in its body, each converted (recursively)
    /// into a `UiRenderNode` carrying resolved `style`/`hover_style` and
    /// best-effort `text` (see ui/render.rs, ui/style.rs, ui/builder.rs).
    ///
    /// Unlike `call_fn`'s UI branch (`call_ui_fn`, still a stub -- see
    /// its comment), this is the one path that actually builds a UI
    /// tree from a named `ui fn`, independent of `run_program`'s
    /// App/Main auto-mount. It always mounts fresh (a new `UiBuilder`
    /// over `ui.body`), so repeated calls don't accumulate state.
    pub fn render_ui_fn(
        &mut self,
        name: &str,
        args: &[EvalValue],
    ) -> EvalResult<Vec<crate::ui::render::UiRenderNode>> {
        self.ui.cache_enabled = false;
        let nodes = self.render_ui_fn_shared(name, args)?;
        Ok(nodes.iter().map(|node| (**node).clone()).collect())
    }

    /// Same as `render_ui_fn`, but keeps the render tree shared and reuses
    /// every subtree whose inputs did not change since the previous call
    /// (see ui/builder/cache.rs). Long-lived sessions render through this.
    pub fn render_ui_fn_reusing(
        &mut self,
        name: &str,
        args: &[EvalValue],
    ) -> EvalResult<Vec<std::rc::Rc<crate::ui::render::UiRenderNode>>> {
        self.ui.cache_enabled = true;
        self.render_ui_fn_shared(name, args)
    }

    /// Shared-tree render that rebuilds everything (no reuse).
    pub(crate) fn render_ui_fn_shared_fresh(
        &mut self,
        name: &str,
        args: &[EvalValue],
    ) -> EvalResult<Vec<std::rc::Rc<crate::ui::render::UiRenderNode>>> {
        self.ui.cache_enabled = false;
        self.render_ui_fn_shared(name, args)
    }

    fn render_ui_fn_shared(
        &mut self,
        name: &str,
        args: &[EvalValue],
    ) -> EvalResult<Vec<std::rc::Rc<crate::ui::render::UiRenderNode>>> {
        let ui = match self.ui_functions.get(name).cloned() {
            Some(ui) => ui,
            None => {
                return Err(tint_evaluator::errors::EvalError::InvalidOp {
                    msg: format!("Unknown ui fn '{}'", name),
                    span: Span::dummy(),
                })
            }
        };

        self.scopes.push();
        for (param, arg) in ui.params.iter().zip(args.iter()) {
            bind_pattern(self, &param.pattern, arg);
        }

        // Same self-borrow swap as `mount_ui` above.
        let mut ui_runtime = std::mem::take(&mut self.ui);
        ui_runtime.mount(&ui, self);
        self.ui = ui_runtime;

        self.scopes.pop();

        let nodes = match self.ui.root {
            Some(root) => crate::ui::render::to_render_tree(&self.ui.tree, root).children,
            None => Vec::new(),
        };

        Ok(nodes)
    }
}
