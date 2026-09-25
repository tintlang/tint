use thiserror::Error;
use rune_ast::Span;

#[derive(Error, Debug)]
pub enum EvalError {
    #[error("unknown identifier `{name}` at {span:?}")]
    UnknownIdent { name: String, span: Span },

    #[error("invalid operation: {msg} at {span:?}")]
    InvalidOp { msg: String, span: Span },

    #[error("call error: {msg} at {span:?}")]
    CallError { msg: String, span: Span },
}

pub type EvalResult<T> = Result<T, EvalError>;
