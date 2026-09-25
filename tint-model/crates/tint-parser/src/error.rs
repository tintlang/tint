
use tint_ast::Span;
use tint_lexer::{TokenKind};

#[derive(Debug)]
pub enum ParserError {
    Unexpected {
        expected: TokenKind,
        found: TokenKind,
        span: Span,
    },

    Message {
        msg: String,
        span: Span,
    },
}

pub type PResult<T> = Result<T, ParserError>;
