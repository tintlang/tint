use crate::Span;
use crate::Stmt;
use crate::pattern::Pattern;
use crate::AttributeList;

use super::{
    stmt::{Block},
    expr::Expr,
    types::Type,
    pattern::{MatchArm},
};

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>, // ["math", "vec3"]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModDecl {
    pub name: String,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub ty: Type,     // сюда попадёт union(i32 | f32 | f64)
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Item {
    Fn(FnDecl),
    UiFn(UiFnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Mod(ModDecl),
    Use(UseDecl),
    GlobalLet(Stmt),

    TypeAlias(TypeAliasDecl),

    ExportFn(FnDecl, Span),
    ExportStruct(StructDecl),
    ExportEnum(EnumDecl),
}

// -----------------------------------------------------------
// FUNCTION DECLARATIONS
// -----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub attributes: AttributeList, 
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub async_: bool,
    pub exported: bool,
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
    pub attributes: AttributeList,   
    pub name: String,
    pub params: Vec<Param>,
    pub body: crate::ui::UiNode,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DefaultValue {
    Single(Expr),      // x {10}
    Broadcast(Expr),   // (a,b,c) {10}
}

#[derive(Debug, Clone)]
pub struct Param {
    pub pattern: Pattern,
    pub ty: Option<Type>, 
    pub default: Option<DefaultValue>,
}

// -----------------------------------------------------------
// STRUCTS & ENUMS
// -----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
    pub exported: bool, 
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StructField {
    // Rust style: name: Type
    Typed {
        name: String,
        ty: Type,
        span: Span,
    },

    // Rune style: name{Type}
    RuneTyped {
        name: String,
        ty: Type,
        span: Span,
    },

    RuneField {   // name
        name: String,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub exported: bool, 
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum EnumVariant {
    Unit(String),
    Tuple(String, Vec<Type>),
    Struct(String, Vec<StructField>),
}

impl StructField {
    pub fn span(&self) -> Span {
        match self {
            StructField::Typed { span, .. } => *span,
            StructField::RuneTyped { span, .. } => *span,
            StructField::RuneField { span, .. } => *span,
        }
    }
}

impl FnDecl {
    /// Ensure parameters do not mix styles:
    /// - no normal param after default-param
    /// - default params can only appear at the END
    pub fn validate(&self) -> Result<(), String> {
        let mut seen_default = false;

        for p in &self.params {
            match &p.default {
                Some(_) => {
                    // first default seen
                    seen_default = true;
                }
                None => {
                    // default was already seen → illegal pattern
                    if seen_default {
                        return Err(format!(
                            "Cannot mix parameters with and without default values in function '{}'",
                            self.name
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}
