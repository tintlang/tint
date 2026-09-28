use super::helpers::*;
use super::*;

impl UiBuilder {
    pub fn new() -> Self {
        Self {
            tree: UiTree::empty(),
            tokens: std::collections::HashMap::new(),
            styles: std::collections::HashMap::new(),
            components: std::collections::HashMap::new(),
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
        self.collect_styles(nodes);
        self.collect_components(nodes);

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
            UiNode::Theme { .. }
            | UiNode::Style { .. }
            | UiNode::Component { .. }
            | UiNode::Variant { .. } => None,
            UiNode::Slot { .. } => None,
            // Splices into the parent instead of creating a node of its
            // own -- handled in `build_into` (nodes.rs), same as `Theme`.
            UiNode::For { .. } => None,
            UiNode::BlockElement {
                name,
                attributes,
                modifiers,
                children,
                ..
            } => self.build_container(name, attributes, modifiers, children, host),
            UiNode::BlockSelfClosing {
                name, modifiers, ..
            } => {
                // Block-mode self-closing nodes (`Tag {}`) carry no
                // `attributes` in the AST at all (see tint_ast::UiNode) --
                // nothing to wire events from here.
                if !check_if(modifiers, host) {
                    return None;
                }
                let expanded = self.expand_styles(modifiers);
                Some(self.tree.create_styled_node_with_host_and_tokens(
                    name.clone(),
                    &expanded,
                    host,
                    &self.tokens,
                ))
            }
        }
    }
}
