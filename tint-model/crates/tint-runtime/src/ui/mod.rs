// ui/runtime.rs

pub mod builder;
pub mod layout;
pub mod render;
pub mod style;
pub mod tree;

use crate::ui::builder::UiBuilder;
use crate::ui::tree::{UiNodeId, UiTree};
use tint_ast::*;
use tint_evaluator::EvalHost;

/// Runtime for the Tint UI system (multi-root, block-mode syntax)
pub struct UiRuntime {
    /// Virtual synthetic root that contains all nodes from UiFnDecl.body
    pub root: Option<UiNodeId>,

    /// Actual UI tree after build / reconcile
    pub tree: UiTree,

    pub dirty: bool,
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRuntime {
    pub fn new() -> Self {
        Self {
            root: None,
            tree: UiTree::empty(),
            dirty: true,
        }
    }

    /// Mount a UI function (ui fn ...)
    ///
    /// `host` is the VM doing the mounting -- building the tree now
    /// evaluates `if{}`/`for{}`/text-interpolation through it (see
    /// builder.rs). Callers that already have `&mut self` on the same
    /// `TintVM` that owns this `UiRuntime` (i.e. `self.ui.mount(...)`)
    /// can't also pass `self` as `host` directly -- that's two overlapping
    /// mutable borrows of the same VM. The fix at those call sites is
    /// `std::mem::take(&mut self.ui)` before calling `mount`, then storing
    /// the result back (see vm.rs's `mount_ui`/`render_ui_fn`).
    pub fn mount<H: EvalHost>(&mut self, ui_fn: &UiFnDecl, host: &mut H) {
        let ast_nodes: &Vec<UiNode> = &ui_fn.body;

        let mut builder = UiBuilder::new();

        // Create synthetic root <Root>
        let synthetic_root = builder.build_root(ast_nodes, host);

        self.root = Some(synthetic_root);
        self.tree = builder.finish();

        self.dirty = true;
    }

    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }

    pub fn render(&mut self) {
        // A GPU backend calls layout::compute_layout(&self.tree, root, w, h)
        // to get every node's real on-screen rect (via taffy, same
        // flexbox algorithm a browser runs), then walks the tree
        // painting each node at that rect -- see ui/layout.rs. The DOM
        // backend (sandbox/) doesn't need this: the browser already does
        // its own layout from the CSS strings ui/style.rs resolves.
        println!("UI RENDER FRAME");

        self.dirty = false;
    }
}
