use crate::attr::AttributeList;
use crate::item::Item;
use crate::Span;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct File {
    pub globals: AttributeList,
    pub items: Vec<Item>,
    // Covers the complete file range, including global attributes and items.
    pub span: Span,
}
