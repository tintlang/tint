#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BorrowKind {
    HighLevel, // borrow(x)
    Group,     // borrow@group(x)
    Strict,    // borrow@strict(x)
}
