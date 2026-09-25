#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowKind {
    HighLevel,   // borrow(x)
    Group,       // borrow@group(x)
    Strict,      // borrow@strict(x)
}
