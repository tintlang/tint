use crate::errors::{SemanticError, SemanticErrorKind};
use crate::prelude::{CheckerContext, Mode};
use crate::type_table::{is_numeric_name, Type};
use std::collections::{HashMap, HashSet};
use tint_ast::{
    Block, EnumVariant, Expr, FnBody, Item, MatchArm, Pattern, PatternField, Program, Span, Stmt,
    StructInitField, UiAttrValue, UiAttribute, UiFnDecl, UiModifier, UiModifierBlock,
    UiModifierItem, UiModifierValue, UiNode, UiNodeOrExpr, UiText, UiTextPart,
};

#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub span: Span,
    pub ty: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Type,
    Variable,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub span: Span,
    pub ty: Type,
    pub kind: SymbolKind,
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticModel {
    pub expressions: Vec<TypedExpr>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    /// Final type of every checked expression, keyed by the address of the
    /// `Expr` node in the `Program` that was checked (see `type_of`). Unlike
    /// spans, addresses stay unique when several files are flattened into one
    /// program.
    pub by_expr: HashMap<usize, Type>,
    /// Resolved signature of every top-level function: parameter types
    /// (unannotated ones inferred) and return type.
    pub functions: HashMap<String, (Vec<Type>, Type)>,
    /// Resolved signature of every method, keyed by (type name, method name).
    /// The receiver is not part of the parameter list.
    pub methods: HashMap<(String, String), (Vec<Type>, Type)>,
    /// Resolved type of every `state` variable and top-level binding.
    pub globals: HashMap<String, Type>,
    /// Resolved type of every name bound by a pattern (`let`, parameters,
    /// match arms, lambda parameters), keyed by the binding's span.
    pub bindings: HashMap<(usize, usize), Type>,
    /// Resolved field types of every struct, by struct name. Generic
    /// parameters stay `Type::Simple("T")`; fields declared without a type
    /// carry the type their uses gave them. Declaration order is the AST's.
    pub struct_fields: HashMap<String, HashMap<String, Type>>,
    /// Resolved payload types of every enum variant, keyed by
    /// (enum, variant), including the built-in `Option` and `Result`.
    pub variant_types: HashMap<(String, String), Vec<Type>>,
    /// Payload field names of every enum variant (`None` for positional ones).
    pub variant_names: HashMap<(String, String), Vec<Option<String>>>,
    /// Type parameters of every generic function (`fn id<T>`), in order. Their
    /// entries in `functions` mention them as `Type::Simple("T")`.
    pub function_generics: HashMap<String, Vec<String>>,
    /// For every call of a generic function, keyed by the address of the
    /// call's target `Expr`: the types its type parameters were inferred as,
    /// in `function_generics` order. Inside another generic function these
    /// may mention that function's own parameters.
    pub instances: HashMap<usize, Vec<Type>>,
}

/// Names exported by the project-wide UI environment.  A `.tn` file can be
/// checked on its own in the editor even though its styles, theme tokens and
/// event handlers are declared in sibling files imported by the entry UI.
#[derive(Debug, Clone, Default)]
pub struct SemanticContext {
    pub(crate) known_fns: HashSet<String>,
    pub(crate) ui_tokens: HashSet<String>,
    pub(crate) ui_styles: HashSet<String>,
    pub(crate) ui_variants: HashSet<String>,
}

impl SemanticModel {
    pub fn type_at(&self, offset: usize) -> Option<&Type> {
        self.expressions
            .iter()
            .filter(|typed| typed.span.start.offset <= offset && offset <= typed.span.end.offset)
            .min_by_key(|typed| typed.span.len())
            .map(|typed| &typed.ty)
    }

    /// Type of one expression node of the program that was checked.
    pub fn type_of(&self, expr: &Expr) -> Option<&Type> {
        self.by_expr.get(&(expr as *const Expr as usize))
    }

    /// True when no expression in the model has an unknown type.
    pub fn is_fully_typed(&self) -> bool {
        self.by_expr.values().all(|ty| !ty.has_unknown())
    }

    pub fn symbol_named(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| symbol.name == name)
    }
}

/// Where an inference variable came from, for `CannotInfer` diagnostics.
#[derive(Debug, Clone)]
struct VarInfo {
    span: Span,
    what: String,
    /// Holes such as `None {}`, `[]` or a diverging branch may stay open;
    /// they default to `unit` once nothing else constrains them.
    default_unit: bool,
}

/// A constraint that cannot be solved until its receiver's type is known.
#[derive(Debug, Clone)]
enum Deferred {
    Method { recv: Type, method: String, args: Vec<Type>, ret: Type, span: Span },
    Field { recv: Type, field: String, ret: Type, span: Span },
    Index { recv: Type, index: Type, ret: Type, span: Span },
    TupleIndex { recv: Type, index: usize, ret: Type, span: Span },
    Try { recv: Type, ret: Type, span: Span },
    Arith { op: String, left: Type, right: Type, ret: Type, spans: (Span, Span) },
}

// Scope management for variable tracking. Frame 0 is never popped -- it
// holds top-level globals (`let` items) for the lifetime of a `check()`
// call, so every function body can see them regardless of declaration
// order, the same way `known_fns` lets functions call each other
// regardless of order.
struct Scopes {
    scopes: Vec<HashMap<String, Type>>,
    mutable: Vec<HashSet<String>>,
}

impl Scopes {
    fn new() -> Self {
        Scopes {
            scopes: vec![HashMap::new()],
            mutable: vec![HashSet::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
        self.mutable.push(HashSet::new());
    }

    fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.mutable.pop();
        }
    }

    fn define(&mut self, name: &str, ty: Type) -> bool {
        self.define_with_mutability(name, ty, false)
    }

    fn define_with_mutability(&mut self, name: &str, ty: Type, is_mutable: bool) -> bool {
        match self.scopes.last_mut() {
            Some(scope) => {
                if scope.contains_key(name) {
                    false
                } else {
                    scope.insert(name.to_string(), ty);
                    if is_mutable {
                        self.mutable.last_mut().unwrap().insert(name.to_string());
                    }
                    true
                }
            }
            None => false,
        }
    }

    /// `let x = ...` again in the same scope shadows the earlier `x`.
    fn redefine(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
            self.mutable.last_mut().unwrap().remove(name);
        }
    }

    fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    fn is_mutable(&self, name: &str) -> bool {
        for (scope, mutable) in self.scopes.iter().rev().zip(self.mutable.iter().rev()) {
            if scope.contains_key(name) {
                return mutable.contains(name);
            }
        }
        false
    }

    fn mark_mutable(&mut self, name: &str) {
        for (scope, mutable) in self.scopes.iter().rev().zip(self.mutable.iter_mut().rev()) {
            if scope.contains_key(name) {
                mutable.insert(name.to_string());
                return;
            }
        }
    }
}

/// Semantic checks over logic and UI bodies. The checker tracks names and
/// inferred types together: operators, calls, fields, methods, assignments,
/// returns, patterns, constructors and UI expressions all use the same type
/// environment. Unknown types stay permissive so untyped legacy code remains
/// usable while declarations and built-in contracts are gradually made more
/// precise.
pub struct SemanticChecker {
    ctx: CheckerContext,
    scopes: Scopes,
    /// Top-level names callable as `name(...)`: every `fn`/`ui fn`, every
    /// struct and enum name (a struct/enum literal isn't lexed as a call,
    /// but costs nothing to allow), every enum variant name (`Some(x)`,
    /// `Ok(v)` parse as an ordinary `Expr::Call` with an `Ident` target,
    /// same as a real function call), and the 3 hardcoded builtins
    /// (`crates/tint-evaluator/src/utils/call.rs`: print/dbg/sqrt). A
    /// call-target identifier is checked against this set, NOT against
    /// `scopes` -- those are two different namespaces, and reusing one
    /// lookup for both is what would make this checker flag every
    /// ordinary function call as an unknown identifier.
    known_fns: HashSet<String>,
    /// The program loads host code (`app { js::"..." }` / `rs::"..."`): unknown
    /// call targets may be functions it exports, which only the host knows.
    host_js: bool,
    ui_tokens: HashSet<String>,
    ui_styles: HashSet<String>,
    ui_variants: HashSet<String>,
    fn_types: HashMap<String, (Vec<Type>, Type)>,
    /// Parameter names and defaults of plain functions, for named arguments.
    fn_sigs: HashMap<String, Vec<tint_ast::ParamSig>>,
    /// Type parameters of generic functions.
    fn_generics: HashMap<String, Vec<String>>,
    /// Inferred type arguments of each call of a generic function, by the
    /// address of the call target (see `SemanticModel::instances`).
    instances: HashMap<usize, Vec<Type>>,
    struct_fields: HashMap<String, HashMap<String, Type>>,
    methods: HashMap<(String, String), (Vec<Type>, Type)>,
    enum_variants: HashMap<(String, String), Vec<Type>>,
    enum_names: HashSet<String>,
    type_names: HashSet<String>,
    generic_arity: HashMap<String, usize>,
    generic_params_by_type: HashMap<String, Vec<String>>,
    generic_params: HashSet<String>,
    current_return: Option<Type>,
    /// Field names of struct-style enum variants (`None` for positional ones),
    /// in declaration order, parallel to `enum_variants`.
    variant_fields: HashMap<(String, String), Vec<Option<String>>>,
    /// Struct fields declared without a type (`struct S { a }`): their type
    /// comes from how they are used, and constructors may leave them out.
    untyped_fields: HashSet<(String, String)>,
    ui_fn_names: HashSet<String>,
    /// For each function ("", name) and method (type, name): which
    /// parameters were written without a type.
    unannotated: HashMap<(String, String), Vec<bool>>,
    /// Unannotated parameters that call sites use with different types.
    /// They stay open (`Unknown`) and strict mode asks for an annotation.
    polymorphic: HashMap<(String, String), Vec<usize>>,
    /// Element type of the `for{}` modifier just visited, for its children.
    for_item: Option<Type>,
    /// Set while binding a `let`: a repeated name shadows instead of clashing.
    rebind_allowed: bool,
    /// Type variable of every top-level `let`, by the span of its pattern.
    global_lets: HashMap<(usize, usize), Type>,
    /// Unification state: `subst[v]` is what inference variable `v` stands for.
    subst: Vec<Option<Type>>,
    var_info: Vec<VarInfo>,
    /// Method calls, field reads, ... whose receiver was not known yet.
    deferred: Vec<Deferred>,
    errors: Vec<SemanticError>,
    inferred: HashMap<(usize, usize), TypedExpr>,
    inferred_by_ptr: HashMap<usize, Type>,
    binding_types: HashMap<(usize, usize), Type>,
    references: Vec<Reference>,
    symbols: Vec<Symbol>,
    external_context: SemanticContext,
}

include!("unify.rs");
include!("names.rs");
include!("patterns.rs");
include!("statements.rs");
include!("expressions.rs");
include!("collections.rs");
include!("ui.rs");
