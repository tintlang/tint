use super::helpers::*;
use super::*;

impl UiBuilder {
    pub fn new() -> Self {
        Self {
            tree: UiTree::empty(),
        }
    }

    /// Create synthetic ROOT (id=0..n)
    ///
    /// `host` is the running VM: building the tree now genuinely evaluates
    /// `if{}`/`for{}` conditions and iterables, and text interpolations,
    /// through it (see `check_if`/`for_loop`/`render_ui_text` below) --
    /// this used to be a pure AST-level pass with no evaluator available,
    /// which is why those constructs were previously inert.
    pub fn build_root<H: EvalHost>(&mut self, nodes: &Vec<UiNode>, host: &mut H) -> UiNodeId {
        let root = self.tree.create_node("Root".into());

        for n in nodes {
            self.build_into(root, n, host);
        }

        root
    }

    /// Convert UiNode AST → UiTree element. Returns `None` when the node
    /// carries an `if{}` modifier that evaluated falsy -- such a node (and
    /// everything under it) simply isn't added to its parent.
    pub fn build<H: EvalHost>(&mut self, node: &UiNode, host: &mut H) -> Option<UiNodeId> {
        match node {
            UiNode::Theme { .. } => None,
            // Splices into the parent instead of creating a node of its
            // own -- handled in `build_into` (nodes.rs), same as `Theme`.
            UiNode::For { .. } => None,
            UiNode::Element {
                name,
                attributes,
                modifiers,
                children,
                ..
            } => self.build_container(name, attributes, modifiers, children, host),
            UiNode::BlockElement {
                name,
                attributes,
                modifiers,
                children,
                ..
            } => self.build_container(name, attributes, modifiers, children, host),
            UiNode::SelfClosing {
                name,
                attributes,
                modifiers,
                ..
            } => {
                if !check_if(modifiers, host) {
                    return None;
                }
                let id = self
                    .tree
                    .create_styled_node_with_host(name.clone(), modifiers, host);
                apply_events(&mut self.tree, id, attributes);
                apply_svg(&mut self.tree, id, attributes);
                apply_route(&mut self.tree, id, attributes);
                apply_asset(&mut self.tree, id, attributes);
                apply_key(&mut self.tree, id, modifiers);
                Some(id)
            }
            UiNode::BlockSelfClosing {
                name, modifiers, ..
            } => {
                // Block-mode self-closing nodes (`Tag {}`) carry no
                // `attributes` in the AST at all (see tint_ast::UiNode) --
                // only XML self-closing (`<Tag />`) does. Nothing to wire
                // events from here.
                if !check_if(modifiers, host) {
                    return None;
                }
                Some(
                    self.tree
                        .create_styled_node_with_host(name.clone(), modifiers, host),
                )
            }
        }
    }
}
