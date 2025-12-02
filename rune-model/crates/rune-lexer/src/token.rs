// crates/lexer/src/token.rs
use rune_ast::{Span, Position};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Identifiers & Literals
    Ident,      // abc, foo, x1
    Number,     // 123, 3.14
    String,     // "text"

    // Brackets & Delimiters
    LBrace,     // {
    RBrace,     // }
    LParen,     // (
    RParen,     // )
    LAngle,     // <
    RAngle,     // >
    LBracket,   // [
    RBracket,   // ]

    Comma,      // ,
    Colon,      // :
    Semicolon,  // ;
    Dot,        // .
    DotDot,     // ..
    DotDotDot,  // ...

    // Arithmetic & Assignment Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %

    Eq,         // =

    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=

    // Comparison Operators
    EqEq,       // ==
    NotEq,      // !=
    Less,       // <
    Greater,    // >
    LessEq,     // <=
    GreaterEq,  // >=

    // Logical Operators
    AndAnd,     // &&
    OrOr,       // ||
    Pipe,       // |
    Bang,       // !

    // Path / Module Operators
    PathSep,    // ::     (module::item)

    // UI Operators
    SlashAngle, // />
    AngleSlash, // </
    FatArrow,   // =>
    Arrow,      // ->

    // Keywords — Logic
    Fn,
    Return,
    Let,
    If,
    Else,
    Match,
    For,
    While,
    Loop,
    Break,
    Continue,
    In,
    Where,

    // inline lambda: |x|
    PipeLambda, // |    <-- UI and Logic pipelines use the same char, but parser distinguishes

    // async model
    Async,
    Await,
    Try,

    // struct & enum & impl
    Struct,
    Enum,
    Impl,
    SelfKw, // "self"

    // Ownership model
    Borrow,
    Immut,
    Move,
    Clone,

    // UI DSL
    Ui,
    State,
    Signal,
    Computed,

    // Keywords — GPU2D DSL
    Rune2d,  

    // Module system
    Module,
    Export,
    Use,   

    // Misc
    Eof,
}

impl TokenKind {
    pub fn binary_precedence(&self) -> u8 {
        match self {
            // assignment-like
            TokenKind::Eq | TokenKind::Colon => 1,

            // logical
            TokenKind::OrOr => 2,
            TokenKind::AndAnd => 3,

            // comparison
            TokenKind::EqEq
            | TokenKind::NotEq
            | TokenKind::Less
            | TokenKind::LessEq
            | TokenKind::Greater
            | TokenKind::GreaterEq => 5,

            // arithmetic
            TokenKind::Plus | TokenKind::Minus => 10,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => 20,

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
