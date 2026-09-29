use tint_ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals and identifiers.
    Ident,
    Number,
    String,

    // Delimiters.
    LBrace,
    RBrace,
    LParen,
    RParen,
    LAngle,
    RAngle,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    DotDot,
    DotDotDot,

    // Arithmetic and assignment operators.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,

    // Comparison and logical operators.
    EqEq,
    NotEq,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    AndAnd,
    Ampersand,
    OrOr,
    Pipe,
    Bang,
    Question,

    // Paths and DSL syntax.
    PathSep,
    SlashAngle,
    AngleSlash,
    FatArrow,
    Arrow,

    // Control flow and declarations.
    Fn,
    Const,
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

    // Literals and patterns.
    True,
    False,
    Underscore,

    // Async execution.
    Async,
    Await,
    Try,

    // User-defined types.
    Struct,
    Enum,
    Impl,
    SelfKw,
    Type,

    // Ownership.
    Borrow,
    Immut,
    Move,
    Clone,

    // Tint UI and GPU DSLs.
    Ui,
    State,
    Signal,
    Computed,
    MapLit,
    Space,
    Kernel,
    Tint2d,

    // Modules.
    Module,
    Export,
    Use,
    Import,
    As,

    At,
    Eof,
}

impl TokenKind {
    pub fn binary_precedence(&self) -> u8 {
        match self {
            Self::OrOr => 1,
            Self::AndAnd => 2,
            Self::EqEq
            | Self::NotEq
            | Self::Less
            | Self::LessEq
            | Self::Greater
            | Self::GreaterEq
            | Self::LAngle
            | Self::RAngle => 5,
            Self::Plus | Self::Minus => 10,
            Self::Star | Self::Slash | Self::Percent => 20,
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
