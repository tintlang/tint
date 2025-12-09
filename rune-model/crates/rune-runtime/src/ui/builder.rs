// ui/builder.rs

use super::tree::{UiTree, UiNodeId};
use rune_ast::UiNode;

pub struct UiBuilder {
    pub tree: UiTree,
}

impl UiBuilder {
    pub fn new() -> Self {
        Self { tree: UiTree::empty() }
    }

    /// Create synthetic ROOT (id=0..n)
    pub fn build_root(&mut self, nodes: &Vec<UiNode>) -> UiNodeId {
        let root = self.tree.create_node("Root".into());

        for n in nodes {
            let child = self.build(n);
            self.tree.add_child(root, child);
        }

        root
    }

    /// Convert UiNode AST → UiTree element
    pub fn build(&mut self, node: &UiNode) -> UiNodeId {
        match node {
            UiNode::Element { name, children, .. } => {
                let id = self.tree.create_node(name.clone());

                for child in children {
                    match child {
                        rune_ast::UiNodeOrExpr::Node(n) => {
                            let cid = self.build(n);
                            self.tree.add_child(id, cid);
                        }
                        rune_ast::UiNodeOrExpr::Text(_) => {
                            // create text node
                            let tid = self.tree.create_node("Text".into());
                            self.tree.add_child(id, tid);
                        }
                    }
                }

                id
            }

            UiNode::BlockElement { name, children, .. } => {
                let id = self.tree.create_node(name.clone());

                for child in children {
                    match child {
                        rune_ast::UiNodeOrExpr::Node(n) => {
                            let cid = self.build(n);
                            self.tree.add_child(id, cid);
                        }
                        rune_ast::UiNodeOrExpr::Text(_) => {
                            let tid = self.tree.create_node("Text".into());
                            self.tree.add_child(id, tid);
                        }
                    }
                }

                id
            }

            UiNode::SelfClosing { name, .. } => {
                self.tree.create_node(name.clone())
            }

            UiNode::BlockSelfClosing { name, .. } => {
                self.tree.create_node(name.clone())
            }
        }
    }

    pub fn finish(self) -> UiTree {
        self.tree
    }
}
