use crate::errors::{SemanticError, SemanticErrorKind};
use crate::prelude::{CheckerContext, Mode};
use std::collections::{HashMap, HashSet};
use tint_ast::{
    Block, EnumVariant, Expr, FnBody, Item, MatchArm, Pattern, PatternField, Program, Span, Stmt,
    StructInitField, Type,
};

// Scope management for variable tracking. Frame 0 is never popped -- it
// holds top-level globals (`let` items) for the lifetime of a `check()`
// call, so every function body can see them regardless of declaration
// order, the same way `known_fns` lets functions call each other
// regardless of order.
struct Scopes {
    scopes: Vec<HashMap<String, Type>>,
}

impl Scopes {
    fn new() -> Self {
        Scopes {
            scopes: vec![HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn define(&mut self, name: &str, ty: Type) -> bool {
        match self.scopes.last_mut() {
            Some(scope) => {
                if scope.contains_key(name) {
                    false
                } else {
                    scope.insert(name.to_string(), ty);
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
}

/// A first, deliberately narrow pass: undefined-variable and
/// duplicate-binding checks over plain `fn` bodies (logic code). It does
/// NOT do type checking (the `TypeMismatch` error kind exists in
/// `errors.rs` for later, nothing constructs it yet) and does NOT walk
/// `ui fn` bodies (a UI tree mixes modifiers/expressions in a shape this
/// pass was never built for -- see the module doc below) or `impl` method
/// bodies (method-call resolution needs the type of `self`, which this
/// pass has no notion of). Both are real, known gaps, not oversights --
/// closing them is separate, larger work.
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
    errors: Vec<SemanticError>,
}

include!("names.rs");
include!("patterns.rs");
include!("statements.rs");
include!("expressions.rs");
