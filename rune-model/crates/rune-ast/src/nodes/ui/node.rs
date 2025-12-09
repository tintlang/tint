use crate::Span;
use super::{UiAttribute, UiModifier, UiText};

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
}

impl UiNode {
    pub fn span(&self) -> Span {
        match self {
            UiNode::Element { span, .. } => *span,
            UiNode::BlockElement { span, .. } => *span,
            UiNode::SelfClosing { span, .. } => *span,
            UiNode::BlockSelfClosing { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiNodeOrExpr {
    Node(UiNode),
    Text(UiText),
}
