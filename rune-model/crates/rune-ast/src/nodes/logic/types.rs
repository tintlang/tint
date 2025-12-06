#[derive(Debug, Clone)]
pub enum Type {
    Simple(String),
    Generic(String, Vec<Type>),
    Unit, 
    Union(Vec<Type>),
}

use crate::Span;

impl Type {
    pub fn span(&self) -> Span {
        Span::dummy()
    }
}