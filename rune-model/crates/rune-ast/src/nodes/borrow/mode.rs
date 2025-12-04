#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowMode {
    Owned,      // x
    Ref,        // &x
    MutRef,     // &mut x
    View,       // UI-only non-owning borrow: <Text>{ value }</Text>
    Weak,       // weak ref for UI or async
}
