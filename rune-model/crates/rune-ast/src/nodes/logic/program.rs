use crate::Span;
use super::item::Item;

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}
