use crate::pattern::Pattern;
use crate::AttributeList;
use crate::Span;
use crate::Stmt;
use crate::UiNode;

use super::{expr::Expr, stmt::Block, types::Type};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UseDecl {
    pub path: Vec<String>, // ["math", "vec3"] -- a module path (`wildcard`) or module::item path
    /// `use a::b as c;` -- import `b` under the local name `c` instead of
    /// its own bare name. Never set together with `wildcard` (there's no
    /// single name to alias a `use a::*` to).
    pub alias: Option<String>,
    /// `use a::*;` -- `path` names a MODULE (not `module::item`); every
    /// exported item in it is imported under its own bare name.
    pub wildcard: bool,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportDecl {
    pub path: String,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModDecl {
    pub name: String,
    /// `mod name { ... }` (inline): populated directly by the parser.
    /// `mod name;` (file-backed, `external == true`): empty right after
    /// parsing -- `tint-cli`'s module loader resolves `name.tn` (or
    /// `name/mod.tn`) relative to the declaring file and fills this in
    /// before the program is checked/run/built. The parser never touches
    /// the filesystem itself.
    pub items: Vec<Item>,
    pub external: bool,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TypeAliasDecl {
    pub name: String,
    pub ty: Type, // union(i32 | f32 | f64)
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Option<Type>,
    pub init: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KernelDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub body: Block, // kernel body
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImplBlock {
    pub span: Span,
    pub generics: Vec<String>, // <T, U>
    pub target: Type,          // Vec3<T>, Texture, List<i32>
    pub methods: Vec<FnDecl>,  // fn foo() {}
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, serde::Serialize, serde::Deserialize)]
pub enum SpaceKind {
    Auto,
    All,
    GPU,
    UI,
    Logic,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpaceDecl {
    pub name: String,
    pub kind: SpaceKind,  // Auto until items parsed
    pub items: Vec<Item>, // fn, kernel, ui fn, struct, impl, etc.
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Item {
    Fn(FnDecl),
    Const(ConstDecl),
    UiFn(UiFnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Mod(ModDecl),
    Use(UseDecl),
    Import(ImportDecl),
    GlobalLet(Stmt),
    Impl(ImplBlock),
    TypeAlias(TypeAliasDecl),
    ExportFn(FnDecl, Span),
    ExportStruct(StructDecl),
    ExportEnum(EnumDecl),
    Kernel(KernelDecl),
    Space(SpaceDecl),
}

// FUNCTION DECLARATIONS
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FnDecl {
    pub attributes: AttributeList,
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub async_: bool,
    pub exported: bool,
    pub body: FnBody,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FnBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiFnDecl {
    pub attributes: AttributeList,
    pub name: String,
    pub params: Vec<Param>,
    /// `state <ident> = <expr>` declarations at the top of the fn body,
    /// before any UI node. Evaluated once when a UI session is created
    /// (see tint-runtime/src/ui_session.rs) and bound into that session's
    /// persistent variable scope -- reading `<ident>` from `if{}`/`for{}`/
    /// text later, and a `click||`/`hover_in||`/`hover_out||` handler
    /// assigning to it, both go through the same scope, which is what
    /// makes a value set by a click visible in the next render.
    pub state: Vec<UiStateDecl>,
    pub body: Vec<UiNode>,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiStateDecl {
    pub name: String,
    pub init: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DefaultValue {
    Single(Expr),    // x {10}
    Broadcast(Expr), // (a,b,c) {10}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub pattern: Pattern,
    pub ty: Option<Type>,
    pub default: Option<DefaultValue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KitRef {
    pub path: Vec<String>, // ["kit", "position2d"]
    pub body: Option<Vec<StructMember>>,
    pub span: Span,
}

impl StructMember {
    pub fn span(&self) -> Span {
        match self {
            StructMember::Kit(k) => k.span,
            StructMember::Field(f) => f.span(),
        }
    }
}

// STRUCTS & ENUMS
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StructMember {
    Field(StructField),
    Kit(KitRef),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub members: Vec<StructMember>,
    pub exported: bool,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StructField {
    // Column style: name: Type
    Typed {
        name: String,
        ty: Type,
        span: Span,
    },

    // Tint style: name{Type}
    TintTyped {
        name: String,
        ty: Type,
        span: Span,
    },

    TintField {
        // name
        name: String,
        span: Span,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnumDecl {
    pub name: String,
    pub generics: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub exported: bool,
    pub span: Span,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EnumVariant {
    Unit(String),
    Tuple(String, Vec<Type>),
    Struct(String, Vec<StructField>),
}

impl StructField {
    pub fn span(&self) -> Span {
        match self {
            StructField::Typed { span, .. } => *span,
            StructField::TintTyped { span, .. } => *span,
            StructField::TintField { span, .. } => *span,
        }
    }
}

impl FnDecl {
    /// Ensure parameters do not mix styles:
    /// -> no normal param after default-param
    /// -> default params can only appear at the END
    pub fn validate(&self) -> Result<(), String> {
        let mut seen_default = false;

        for p in &self.params {
            match &p.default {
                Some(_) => {
                    // first default seen
                    seen_default = true;
                }
                None => {
                    // default was already seen -> illegal pattern
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
