#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Type {
    Simple(String),
    Generic(String, Vec<Type>),
    Unit,
    Union(Vec<Type>),
    /// A first-class function type: `fn(i32, f32) -> bool`.
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
}

use crate::Span;

impl Type {
    /// Types do not currently retain source locations, so callers receive a dummy span.
    pub fn span(&self) -> Span {
        Span::dummy()
    }
}
