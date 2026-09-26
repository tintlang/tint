use super::item::Item;
use crate::AttributeList;
use crate::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub globals: AttributeList,
    pub items: Vec<Item>,
}
