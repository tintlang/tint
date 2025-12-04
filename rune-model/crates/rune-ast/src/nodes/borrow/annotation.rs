use super::{BorrowMode, LifetimeId};

#[derive(Debug, Clone)]
pub struct BorrowAnnotation {
    pub mode: BorrowMode,
    pub lifetime: LifetimeId,
}
