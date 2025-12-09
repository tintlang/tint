// ui/runtime.rs

pub mod tree;
pub mod builder;
pub mod reconciler;
pub mod events;
pub mod layout;
pub mod render;

use crate::vm::RuneVM;
use rune_ast::*;
use crate::ui::tree::{UiNodeId, UiTree};
use crate::ui::builder::UiBuilder;
use crate::ui::events::UiEventSystem;

/// Runtime for the new Rune UI system (multi-root, XML or BLOCK mode)
pub struct UiRuntime {
    /// Virtual synthetic root that contains all nodes from UiFnDecl.body
    pub root: Option<UiNodeId>,

    /// Actual UI tree after build / reconcile
    pub tree: UiTree,

    pub events: UiEventSystem,
    pub dirty: bool,
}

impl UiRuntime {
    pub fn new() -> Self {
        Self {
            root: None,
            tree: UiTree::empty(),
            events: UiEventSystem::new(),
            dirty: true,
        }
    }

    /// Mount a UI function (ui fn ...)
    /// NEW: supports ui_fn.body = Vec<UiNode>
    pub fn mount(&mut self, ui_fn: &UiFnDecl) {
        let ast_nodes: &Vec<UiNode> = &ui_fn.body;

        let mut builder = UiBuilder::new();

        // Create synthetic root <Root>
        let synthetic_root = builder.build_root(ast_nodes);

        self.root = Some(synthetic_root);
        self.tree = builder.finish();

        self.dirty = true;
    }

    /// Handle input events + triggers dirty flag if needed
    pub fn process_events(&mut self, vm: &mut RuneVM) -> bool {
        self.events.dispatch(&mut self.tree, vm);
        self.dirty
    }

    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }

    pub fn render(&mut self) {
        // TODO: integrate with Lynbor WebGPU real renderer
        println!("UI RENDER FRAME");

        // tree.render() → your WebGPU backend
        // layout::compute(&mut tree)
        // apply animations

        self.dirty = false;
    }
}
