use crate::errors::{SemanticError, SemanticErrorKind};
use crate::prelude::{CheckerContext, Mode};
use crate::type_table::Type;
use std::collections::{HashMap, HashSet};
use tint_ast::{
    Block, EnumVariant, Expr, FnBody, Item, MatchArm, Pattern, PatternField, Program, Span, Stmt,
    StructInitField, UiAttrValue, UiAttribute, UiFnDecl, UiModifier, UiModifierBlock,
    UiModifierItem, UiModifierValue, UiNode, UiNodeOrExpr, UiText, UiTextPart,
};

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

    fn update_type(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), ty);
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
    ui_tokens: HashSet<String>,
    ui_styles: HashSet<String>,
    ui_variants: HashSet<String>,
    fn_types: HashMap<String, (Vec<Type>, Type)>,
    struct_fields: HashMap<String, HashMap<String, Type>>,
    methods: HashMap<(String, String), (Vec<Type>, Type)>,
    enum_variants: HashMap<(String, String), Vec<Type>>,
    enum_names: HashSet<String>,
    type_names: HashSet<String>,
    current_return: Option<Type>,
    errors: Vec<SemanticError>,
}

include!("names.rs");
include!("patterns.rs");
include!("statements.rs");
include!("expressions.rs");
include!("ui.rs");
