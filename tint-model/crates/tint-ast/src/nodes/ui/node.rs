use super::{UiAttribute, UiModifier, UiText};
use crate::logic::expr::Expr;
use crate::Span;

#[derive(Debug, Clone)]
pub enum UiNode {
    // BLOCK-style Tag { ... }
    //
    // Also the shape a `case <label> { ... }` arm parses to (a
    // `BlockElement` named "case" carrying a synthetic
    // `UiAttribute{name: "case", ...}` -- see
    // tint-parser/src/ui/block.rs's `parse_block_case_node`), and the
    // shape a `children { ... }` grouping tag parses to (nothing special
    // in the AST at all -- it's an ordinary tag named "children" that the
    // builder splices instead of wrapping, see
    // tint-runtime/src/ui/builder/nodes.rs). "children" is therefore an
    // unconditionally reserved tag name at the builder level -- an
    // ordinary component actually named `children` always gets spliced
    // instead of rendering as its own node ("case" isn't reserved the
    // same way: `case { ... }` with no label in front still renders as a
    // plain tag, only `case <label> { ... }` is special -- see the parser
    // vs. builder doc comments for each).
    BlockElement {
        name: String,
        attributes: Vec<UiAttribute>,
        modifiers: Vec<UiModifier>,
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    // BLOCK-style self closing: Tag {}
    BlockSelfClosing {
        name: String,
        modifiers: Vec<UiModifier>,
        span: Span,
    },

    /// A theme-scoped UI fragment: `theme::dark { ... }`.
    Theme {
        name: String,
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    /// A standalone repeated block: `for { var in iterable } { ...body... }`.
    /// Unlike the `for{}` modifier on `UiModifier` (attached to a single
    /// node, repeating THAT node's own children as if it were a template),
    /// this is a child in its own right, same as `Theme` above -- it can
    /// sit among static siblings and, per iteration, splices its whole
    /// `body` directly into the parent, no wrapper node of its own.
    For {
        var: String,
        iterable: Expr,
        body: Vec<UiNodeOrExpr>,
        span: Span,
    },
}

impl UiNode {
    pub fn span(&self) -> Span {
        match self {
            UiNode::BlockElement { span, .. } => *span,
            UiNode::BlockSelfClosing { span, .. } => *span,
            UiNode::Theme { span, .. } => *span,
            UiNode::For { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiNodeOrExpr {
    Node(UiNode),
    Text(UiText),
}
