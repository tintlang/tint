use tint_ast::span::Span;

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub span: Span,
    pub kind: SemanticErrorKind,
}

#[derive(Debug, Clone)]
pub enum SemanticErrorKind {
    UnknownIdent(String),
    UnknownUiToken(String),
    UnknownUiStyle(String),
    UnknownUiVariant(String),
    DuplicateIdent(String),
    AssignToImmutable(String),
    TypeMismatch { expected: String, found: String },
    InvalidUiInLogic,
    InvalidLogicInUi,
    InvalidAttributeSyntax,
    InvalidModifierSyntax,
    MissingElseInBlockIf,
    ElseOutsideIf,
    BorrowInUiMode,
    NestedBorrow,
    MissingMatchArms,
    WrongEventBinding,
    Unsupported,
}

impl SemanticError {
    pub fn new(kind: SemanticErrorKind, span: Span) -> Self {
        Self { span, kind }
    }
}
