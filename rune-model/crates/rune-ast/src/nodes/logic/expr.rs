use crate::Span;
use super::stmt::Block;
use crate::MatchArm;
use crate::BorrowKind;

#[derive(Debug, Clone)]
pub enum StructInitField {
    Assign { name: String, expr: Expr, span: Span }, // name: expr
    Rune { name: String, expr: Expr, span: Span },   // name{expr}
}

impl StructInitField {
    pub fn span(&self) -> Span {
        match self {
            StructInitField::Assign { span, .. } => *span,
            StructInitField::Rune   { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructInit {
    pub name: String,
    pub fields: Vec<StructInitField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StringPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(String, Span),
    String(String, Span), // raw literal (no interpolation)
    Bool(bool, Span),  
    Unit(Span),   
    Ident(String, Span),
    SelfKw(Span),

    InterpolatedString {
        parts: Vec<StringPart>,
        span: Span,
    },

    Call {
        target: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },

    Field {
        target: Box<Expr>,
        field: String,
        span: Span,
    },

    Array {
    items: Vec<Expr>,
    span: Span,
    },

    Namespace {
        base: Box<Expr>,
        item: String,
        span: Span,
    },

    Index {
        target: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },

    Unary {
        op: String,
        expr: Box<Expr>,
        span: Span,
    },

    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
        span: Span,
    },

    Paren(Box<Expr>, Span),

    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },

    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        span: Span,
    },

    StructInit {
        name: String,
        fields: Vec<StructInitField>,
        span: Span,
    },

    StructUpdate {
        base: Box<Expr>,
        updates: Vec<StructInitField>,
        span: Span,
    },

    NamedArg {
        name: String,
        value: Box<Expr>,
        span: Span,
    },

        /// { stmt… }
    Block(Block, Span),

    Tuple {
        items: Vec<Expr>,
        span: Span,
    },
    
    TupleIndex {
        target: Box<Expr>,
        index: usize,
        span: Span,
    },

    VariantInit {
        enum_name: String,
        variant: String,
        fields: Vec<StructInitField>,
        span: Span,
    },

    MapInit {
        entries: Vec<(String, Expr)>,
        span: Span,
    },

       Borrow {
        kind: BorrowKind,
        target: Box<Expr>,
        block: Option<Block>,   // borrow(x) { ... }
        span: Span,
    },

}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s)
            | Expr::String(_, s)
            | Expr::Ident(_, s)
            | Expr::Bool(_, s)
            | Expr::Unit(s)
            | Expr::SelfKw(s)
            | Expr::Paren(_, s) => *s,

            Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Field { span, .. }
            | Expr::Array { span, .. }
            | Expr::Namespace { span, .. }
            | Expr::Index { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::InterpolatedString { span, .. }
            | Expr::StructInit { span, .. }
            | Expr::StructUpdate { span, .. }
            | Expr::NamedArg { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::Block(_, span)
            | Expr::Match { span, .. } => *span,
            Expr::TupleIndex { span, .. } => *span,
            Expr::VariantInit { span, .. } => *span,
            Expr::MapInit { span, .. } => *span,
            Expr::Borrow { span, .. } => *span,
        }
    }
}
