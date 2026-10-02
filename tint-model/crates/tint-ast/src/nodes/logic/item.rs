use crate::pattern::Pattern;
use crate::AttributeList;
use crate::Span;
use crate::Stmt;
use crate::UiModifier;
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

/// `app { title::"Tint Pong", lang::"en" }` -- page metadata declared in the
/// program itself, so no hand-written HTML `<head>` is needed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppDecl {
    pub modifiers: Vec<UiModifier>,
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
    App(AppDecl),
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
    /// `derived total = price * qty`: not state but a name for an expression of the view (the
    /// parser substitutes it where it is used, see tint-parser/src/components.rs).
    #[serde(default)]
    pub derived: bool,
    /// `persist state name = "x"`: a string state kept in storage (see components.rs).
    #[serde(default)]
    pub persist: bool,
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

impl DefaultValue {
    /// The expression that is evaluated when the argument is omitted.
    pub fn expr(&self) -> &Expr {
        match self {
            DefaultValue::Single(e) | DefaultValue::Broadcast(e) => e,
        }
    }
}

impl Param {
    /// The bound name of a plain parameter (`x`, `mut x`, `x: T`).
    pub fn name(&self) -> Option<&str> {
        fn walk(p: &Pattern) -> Option<&str> {
            match p {
                Pattern::Ident(name, _) => Some(name),
                Pattern::Mut { inner, .. } => walk(inner),
                Pattern::Typed { pat, .. } => walk(pat),
                _ => None,
            }
        }
        walk(&self.pattern)
    }

    pub fn sig(&self) -> ParamSig {
        ParamSig { name: self.name().map(str::to_owned), has_default: self.default.is_some() }
    }
}

/// What a call site needs to know about a parameter.
#[derive(Debug, Clone)]
pub struct ParamSig {
    pub name: Option<String>,
    pub has_default: bool,
}

/// Matches call arguments to parameters: positional arguments fill the
/// parameters in order, `name: value` arguments (`Expr::NamedArg`) fill by
/// name, and any parameter left over must have a default. Returns, per
/// parameter, the index of its argument in `args`, or `None` when the
/// default applies. `Expr::NamedArg` values are the caller's to unwrap.
pub fn bind_call_args(
    fn_name: &str,
    params: &[ParamSig],
    args: &[Expr],
) -> Result<Vec<Option<usize>>, String> {
    let mut slots: Vec<Option<usize>> = vec![None; params.len()];
    let mut next = 0;
    for (index, arg) in args.iter().enumerate() {
        match arg {
            Expr::NamedArg { name, .. } => {
                let Some(pos) = params.iter().position(|p| p.name.as_deref() == Some(name)) else {
                    return Err(format!("`{fn_name}` has no parameter `{name}`"));
                };
                if slots[pos].is_some() {
                    return Err(format!("argument `{name}` of `{fn_name}` is given twice"));
                }
                slots[pos] = Some(index);
            }
            _ => {
                if next >= params.len() {
                    return Err(format!(
                        "`{fn_name}` takes {} arguments, got {}",
                        params.len(),
                        args.len()
                    ));
                }
                if slots[next].is_some() {
                    return Err(format!("argument {} of `{fn_name}` is given twice", next + 1));
                }
                slots[next] = Some(index);
                next += 1;
            }
        }
    }
    for (param, slot) in params.iter().zip(&slots) {
        if slot.is_none() && !param.has_default {
            return Err(match &param.name {
                Some(name) => format!("missing argument `{name}` in call of `{fn_name}`"),
                None => format!("missing argument in call of `{fn_name}`"),
            });
        }
    }
    Ok(slots)
}

/// The value expression of an argument, without a `name:` wrapper.
pub fn arg_value(arg: &Expr) -> &Expr {
    match arg {
        Expr::NamedArg { value, .. } => value,
        other => other,
    }
}
