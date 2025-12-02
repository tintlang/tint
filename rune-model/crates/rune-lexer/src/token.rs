// crates/lexer/src/token.rs
use rune_ast::{Span, Position};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // identifiers & literals
    Ident,
    Number,
    String,

    // symbols
    LBrace,     // {
    RBrace,     // }
    LParen,     // (
    RParen,     // )
    LAngle,     // <
    RAngle,     // >
    SlashAngle, // />
    AngleSlash, // </
    Comma,      // ,
    Colon,      // :
    Eq,         // =
    Arrow,      // ->
    FatArrow,   // =>
    DotDot,     // ..
    Semicolon,  // ;

    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /

    // keywords
    Fn,
    Ui,
    Let,
    State,
    Signal,
    Computed,
    Match,
    For,
    If,
    Else,
    Async,
    Await,
    Module,
    Export,
    Enum,
    Struct,
    Borrow,
    Immut,
    Try,
    Move,
    Clone,

    // misc
    Eof,
}

impl TokenKind {
    pub fn binary_precedence(&self) -> u8 {
        match self {
            // assignment-like
            TokenKind::Eq | TokenKind::Colon => 1,

            // arithmetic
            TokenKind::Plus | TokenKind::Minus => 10,
            TokenKind::Star | TokenKind::Slash => 20,

            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub lexeme: String,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span, lexeme: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            lexeme: lexeme.into(),
        }
    }
}
