// rune-ast/file.rs

use crate::attr::AttributeList;
use crate::item::Item;
use crate::Span;

#[derive(Debug, Clone)]
pub struct File {
    pub globals: AttributeList,
    pub items: Vec<Item>,
    pub span: Span,   // можно вычислять по первому/последнему item/attr
}
