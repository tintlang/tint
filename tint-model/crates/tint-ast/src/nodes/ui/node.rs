use super::{UiAttribute, UiModifier, UiText};
use crate::Span;

#[derive(Debug, Clone)]
pub enum UiNode {
    // XML-style <Tag ...>...</Tag>
    Element {
        name: String,
        attributes: Vec<UiAttribute>,
        modifiers: Vec<UiModifier>,
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    // BLOCK-style Tag { ... }
    BlockElement {
        name: String,
        attributes: Vec<UiAttribute>,
        modifiers: Vec<UiModifier>,
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    // XML-style <Tag ... />
    SelfClosing {
        name: String,
        attributes: Vec<UiAttribute>,
        modifiers: Vec<UiModifier>,
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
}

impl UiNode {
    pub fn span(&self) -> Span {
        match self {
            UiNode::Element { span, .. } => *span,
            UiNode::BlockElement { span, .. } => *span,
            UiNode::SelfClosing { span, .. } => *span,
            UiNode::BlockSelfClosing { span, .. } => *span,
            UiNode::Theme { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiNodeOrExpr {
    Node(UiNode),
    Text(UiText),
}
