use crate::Span;

// ============================================================
// PROGRAM
// ============================================================

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


// ============================================================
// FUNCTION DECLARATIONS
// ============================================================

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub async_: bool,
    pub body: FnBody,
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


// ============================================================
// BLOCKS & STATEMENTS
// ============================================================

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, expr: Expr, span: Span },
    Assign { name: String, expr: Expr, span: Span },
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


// ============================================================
// PATTERNS & MATCH
// ============================================================

#[derive(Debug, Clone)]
pub enum Pattern {
    Ident(String, Span),
    Number(String, Span),
    String(String, Span),

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


// ============================================================
// EXPRESSIONS
// ============================================================

#[derive(Debug, Clone)]
pub enum Expr {
    Number(String, Span),
    String(String, Span),
    Ident(String, Span),

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

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s)
            | Expr::String(_, s)
            | Expr::Ident(_, s)
            | Expr::Paren(_, s) => *s,

            Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Field { span, .. }
            | Expr::Namespace { span, .. }
            | Expr::Index { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::StructInit { span, .. } => *span,
        }
    }
}


// ============================================================
// TYPES
// ============================================================

#[derive(Debug, Clone)]
pub enum Type {
    Simple(String),
    Generic(String, Vec<Type>),
}


// ============================================================
// UI TEXT WITH INTERPOLATION
// ============================================================

#[derive(Debug, Clone)]
pub enum UiTextPart {
    Literal(String, Span),
    Interpolation(Expr, Span),
}

#[derive(Debug, Clone)]
pub struct UiText {
    pub parts: Vec<UiTextPart>,
    pub span: Span,
}


// ============================================================
// UI NODES
// ============================================================

#[derive(Debug, Clone)]
pub enum UiNode {
    Element {
        name: String,
        attributes: Vec<UiAttribute>,      // event handlers, props
        modifiers: Vec<UiModifier>,        // padding{}, radius{}, animate{}
        children: Vec<UiNodeOrExpr>,
        span: Span,
    },

    SelfClosing {
        name: String,
        attributes: Vec<UiAttribute>,
        modifiers: Vec<UiModifier>,
        span: Span,
    },
}


impl UiNode {
    pub fn span(&self) -> Span {
        match self {
            UiNode::Element { span, .. } => *span,
            UiNode::SelfClosing { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum UiNodeOrExpr {
    Node(UiNode),
    Text(UiText),
}


// ============================================================
// UI ATTRIBUTES & MODIFIERS
// ============================================================

#[derive(Debug, Clone)]
pub struct UiAttribute {
    pub name: String,
    pub value: UiAttrValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UiAttrValue {
    Literal(String),        // attr="text"
    Ident(String),          // attr=foo
    Expr(Expr),             // attr={a + b}
    Modifier(UiModifierBlock),
}


// ============================================================
// UI MODIFIER BLOCK
// ============================================================

#[derive(Debug, Clone)]
pub struct UiModifierBlock {
    pub path: Vec<String>,          // padding.x → ["padding","x"]
    pub items: Vec<UiModifierItem>, // inside { ... }
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UiModifierItem {
    pub key: String,
    pub value: Option<UiModifierValue>,
    pub children: Vec<UiModifierItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UiModifier {
    pub path: Vec<String>,    // padding.x -> ["padding","x"]
    pub value: UiModifierValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum UiModifierValue {
    Number(f64),
    String(String),
    Expr(Expr),          // animate{ opacity: 0 -> 1 }
    Block(Vec<UiModifier>),  // nested modifiers
}

// ============================================================
// STRUCTS & ENUMS
// ============================================================

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