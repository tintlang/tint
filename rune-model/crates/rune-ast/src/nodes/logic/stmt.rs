use crate::Span;
use super::pattern::Pattern; 
use super::{
    expr::Expr,
    pattern::{MatchArm},
    types::Type,
};

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LetInit {
    Assign(Expr),   // let a = expr
    Rune(Expr),     // let a{expr}
}

impl LetInit {
    pub fn span(&self) -> Span {
        match self {
            LetInit::Assign(expr) => expr.span(),
            LetInit::Rune(expr) => expr.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        pattern: Pattern,
        ty: Option<Type>,   // let a: i32 = 10
        init: LetInit,      // (=expr) или {expr}
        span: Span,
    },

    Assign {
        lhs: Expr,
        rhs: Expr,
        span: Span,
    },

    CompoundAssign {
        name: String,
        op: String,
        expr: Expr,
        span: Span,
    },

    Expr(Expr),

    If {
        cond: Expr,
        then: Block,
        else_: Option<Block>,
        span: Span,
    },

    While {
        cond: Expr,
        body: Block,
        span: Span,
    },

    Loop {
        body: Block,
        span: Span,
    },

    Break(Span),
    Continue(Span),

    For {
        var: String,
        start: Expr,
        end: Expr,
        body: Block,
        span: Span,
    },

    Match {
        expr: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },

    Return(Expr, Span),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. } => *span,
            Stmt::Assign { span, .. } => *span,
            Stmt::CompoundAssign { span, .. } => *span,
            Stmt::Expr(expr) => expr.span(),
            Stmt::If { span, .. } => *span,
            Stmt::While { span, .. } => *span,
            Stmt::Loop { span, .. } => *span,
            Stmt::Break(span) => *span,
            Stmt::Continue(span) => *span,
            Stmt::For { span, .. } => *span,
            Stmt::Match { span, .. } => *span,
            Stmt::Return(_, span) => *span,
        }
    }
}
