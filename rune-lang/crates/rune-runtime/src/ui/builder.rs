// ui/builder.rs

use super::tree::{UiTree, UiElement, UiNodeId};
use rune_ast::UiNode;

pub struct UiBuilder {
    tree: UiTree,
}

impl UiBuilder {
    pub fn new() -> Self {
        Self { tree: UiTree { nodes: vec![] } }
    }

    pub fn build(&mut self, _root: &UiNode) -> UiNodeId {
        // TODO: AST -> UiTree
        0
    }

    pub fn finish(self) -> UiTree {
        self.tree
    }
}
