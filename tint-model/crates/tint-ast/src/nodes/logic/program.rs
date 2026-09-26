use super::item::Item;
use crate::AttributeList;

#[derive(Debug, Clone)]
pub struct Program {
    pub globals: AttributeList,
    pub items: Vec<Item>,
}
