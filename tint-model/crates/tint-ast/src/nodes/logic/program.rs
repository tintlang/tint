use super::item::Item;
use crate::AttributeList;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Program {
    pub globals: AttributeList,
    pub items: Vec<Item>,
}
