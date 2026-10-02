use super::stmt::Block;
use crate::BorrowKind;
use crate::MatchArm;
use crate::Span;
use crate::Type;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StructInitField {
    Assign {
        name: String,
        expr: Expr,
        span: Span,
    }, // name: expr
    Tint {
        name: String,
        expr: Expr,
        span: Span,
    }, // name{expr}
}

impl StructInitField {
    pub fn span(&self) -> Span {
        match self {
            StructInitField::Assign { span, .. } => *span,
            StructInitField::Tint { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructInit {
    pub name: String,
    pub fields: Vec<StructInitField>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StringPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(not(feature = "lean-expr"), derive(serde::Deserialize))]
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

    /// Conditional expression: `if condition { value } else { value }`.
    If {
        cond: Box<Expr>,
        then: Block,
        else_: Block,
        span: Span,
    },

    Lambda {
        params: Vec<String>,
        /// Written types of the parameters (`|r: Result<number, string>|`); empty when none is written.
        param_types: Vec<Option<Type>>,
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

    /// A block used as an expression: `{ stmt... }`.
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
        // Some borrow forms may own a scoped block: `borrow(x) { ... }`.
        block: Option<Block>,
        span: Span,
    },

    Try {
        expr: Box<Expr>,
        span: Span,
    },

    Cast {
        expr: Box<Expr>,
        ty: Type,
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
            | Expr::Match { span, .. }
            | Expr::If { span, .. } => *span,
            Expr::TupleIndex { span, .. } => *span,
            Expr::VariantInit { span, .. } => *span,
            Expr::MapInit { span, .. } => *span,
            Expr::Borrow { span, .. } => *span,
            Expr::Try { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
        }
    }
}

/// With `lean-expr` an expression read back from JSON is skipped and replaced by
/// a unit placeholder. The runtime of a page compiled to WebAssembly only needs
/// the *position* of each expression inside a UI template (its value arrives in
/// a slot, see `tint_ir::typed::ui`), and not carrying the deserializer for the
/// whole expression language saves about a fifth of that runtime.
#[cfg(feature = "lean-expr")]
impl<'de> serde::Deserialize<'de> for Expr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(deserializer)?;
        let at = crate::Position { offset: 0, line: 1, column: 1 };
        Ok(Expr::Unit(Span::new(at, at)))
    }
}
