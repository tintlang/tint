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

pub struct UiRuntime {
    pub root: Option<UiNodeId>,
    pub tree: UiTree,
    pub events: UiEventSystem,
    pub dirty: bool,
}

impl UiRuntime {
    pub fn new() -> Self {
        Self {
            root: None,
            tree: UiTree { nodes: vec![] },
            events: UiEventSystem::new(),
            dirty: true,
        }
    }

    pub fn mount(&mut self, ui_fn: &UiFnDecl) {
        let root_ast = &ui_fn.body;

        let mut builder = UiBuilder::new();
        let root_id = builder.build(root_ast);

        self.root = Some(root_id);
        self.tree = builder.finish();
        self.dirty = true;
    }

    pub fn process_events(&mut self) -> bool {
        // TODO: real event handling
        false
    }

    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }

    pub fn render(&mut self) {
        // TODO: integrate with Lynbor WebGPU
        println!("UI RENDER");
        self.dirty = false;
    }
}
