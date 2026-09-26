use super::expr::Expr;
use crate::Span;
use crate::StructInitField;
use crate::Type;

#[derive(Debug, Clone)]
pub enum Pattern {
    Ident(String, Span),
    Number(String, Span),
    String(String, Span),
    Wildcard(Span),

    // (x, y, z)
    Tuple(Vec<Pattern>, Span),

    // User { id, name, .. }
    Struct {
        name: String,
        fields: Vec<PatternField>,
        span: Span,
    },

    // Some(x), Ok(v), Error(msg, code)
    Variant {
        name: String,
        args: Vec<Pattern>,
        span: Span,
    },

    Typed {
        pat: Box<Pattern>,
        ty: Type,
        span: Span,
    },

    Map {
        fields: Vec<PatternField>,
        span: Span,
    },

    Mut {
        inner: Box<Pattern>,
        span: Span,
    },

    Group {
        fields: Vec<PatternField>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum PatternField {
    // field
    Shorthand {
        field: String,
        span: Span,
    },

    // field: bind
    Assign {
        field: String,
        pat: Pattern,
        span: Span,
    },

    // ..
    Rest(Span),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub expr: Expr,
    pub span: Span,
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Ident(_, s)
            | Pattern::Number(_, s)
            | Pattern::String(_, s)
            | Pattern::Wildcard(s)
            | Pattern::Tuple(_, s)
            | Pattern::Struct { span: s, .. }
            | Pattern::Variant { span: s, .. }
            | Pattern::Typed { span: s, .. }
            | Pattern::Map { span: s, .. } => *s,
            Pattern::Mut { span, .. } => *span,
            Pattern::Group { span, .. } => *span,
        }
    }
}

impl PatternField {
    pub fn span(&self) -> Span {
        match self {
            PatternField::Shorthand { span, .. } => *span,
            PatternField::Assign { span, .. } => *span,
            PatternField::Rest(span) => *span,
        }
    }
}
