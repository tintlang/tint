#[derive(Debug, Clone)]
pub enum Type {
    Simple(String),
    Generic(String, Vec<Type>),
    Unit, 
    Union(Vec<Type>),
}

use crate::Span;

impl Type {
    /// Types do not currently retain source locations, so callers receive a dummy span.
    pub fn span(&self) -> Span {
        Span::dummy()
    }
}