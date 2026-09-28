use super::{BorrowKind, LifetimeId};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BorrowAnnotation {
    pub kind: BorrowKind,

    /// Present only for strict borrows, which are tied to an explicit lifetime region.
    pub lifetime: Option<LifetimeId>,
}
