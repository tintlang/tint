use super::{BorrowKind, LifetimeId};

#[derive(Debug, Clone)]
pub struct BorrowAnnotation {
    pub kind: BorrowKind,

    // For strict borrow only
    pub lifetime: Option<LifetimeId>,
}
