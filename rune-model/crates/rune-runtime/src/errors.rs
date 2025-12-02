use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("Unknown identifier")]
    UnknownIdent,

    #[error("Invalid operation")]
    InvalidOp,

    #[error("Borrow conflict")]
    BorrowConflict,

    #[error("Undefined function")]
    NoSuchFunction,
}
