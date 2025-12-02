use crate::Span;

// Program
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnDecl),
    UiFn(UiFnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
}

// Functions
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,     // optional return type
    pub async_: bool,             // async fn
    pub body: FnBody,             // Block or Expr
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum FnBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct UiFnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub body: UiNode,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

// Blocks & Statements
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, expr: Expr, span: Span },
    Assign { name: String, expr: Expr, span: Span },
    // +=, -=, *=, /= 
    CompoundAssign { name: String, op: String, expr: Expr, span: Span },
    Expr(Expr),
    If { cond: Expr, then: Block, else_: Option<Block>, span: Span },
    While { cond: Expr, body: Block, span: Span },
    Loop { body: Block, span: Span },
    Break(Span),
    Continue(Span),
    For { var: String, start: Expr, end: Expr, body: Block, span: Span },
    Match { expr: Expr, arms: Vec<MatchArm>, span: Span },
    Return(Expr, Span),
}


// Match patterns
#[derive(Debug, Clone)]
pub enum Pattern {
    Ident(String, Span),
    Number(String, Span),
    String(String, Span),

    // enum variant: Variant(a,b)
    Variant {
        name: String,
        args: Vec<Pattern>,
        span: Span,
    },

    Wildcard(Span),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub expr: Expr,
    pub span: Span,
}

// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    Number(String, Span),
    String(String, Span),
    Ident(String, Span),

    // foo(arg1, arg2)
    Call {
        target: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },

    // a.b
    Field {
        target: Box<Expr>,
        field: String,
        span: Span,
    },

    // a::b
    Namespace {
        base: Box<Expr>,
        item: String,
        span: Span,
    },

    // arr[i]
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

    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        span: Span,
    },

    StructInit {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
}

// Types
#[derive(Debug, Clone)]
pub enum Type {
    Simple(String),
    Generic(String, Vec<Type>),
}

// UI nodes
#[derive(Debug, Clone)]
pub enum UiNode {
    Element {
        name: String,
        attributes: Vec<UiAttribute>,
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    SelfClosing {
        name: String,
        attributes: Vec<UiAttribute>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum UiNodeOrExpr {
    Node(UiNode),
    Text(String, Span),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct UiAttribute {
    pub name: String,
    pub value: UiAttrValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UiAttrValue {
    Ident(String),
    Literal(String),
    Interpolation(Expr),
    Modifier(Vec<ModifierItem>),
}

#[derive(Debug, Clone)]
pub struct ModifierItem {
    pub key: String,
    pub value: Option<String>,
    pub children: Vec<ModifierItem>,
    pub span: Span,
}

// Struct & Enum
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum EnumVariant {
    Unit(String),
    Tuple(String, Vec<Type>),
    Struct(String, Vec<StructField>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, span)
            | Expr::String(_, span)
            | Expr::Ident(_, span)
            | Expr::Paren(_, span)
            => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Field { span, .. } => *span,
            Expr::Namespace { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Lambda { span, .. } => *span,
            Expr::StructInit { span, .. } => *span,
        }
    }
}


impl UiNode {
    pub fn span(&self) -> Span {
        match self {
            UiNode::Element { span, .. } => *span,
            UiNode::SelfClosing { span, .. } => *span,
        }
    }
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