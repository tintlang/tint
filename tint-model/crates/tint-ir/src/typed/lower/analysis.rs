//! Syntax walkers and the "does this method change its receiver" analysis.

use super::*;
use tint_ast::{Block, Expr, LetInit, MatchArm, StringPart, StructInitField};

pub(crate) enum Node<'a> {
    Stmt(&'a Stmt),
    Expr(&'a Expr),
}

pub(crate) fn walk_block(block: &Block, f: &mut dyn FnMut(Node) -> bool) -> bool {
    block.stmts.iter().any(|s| walk_stmt(s, f))
}

/// Visits `stmt` and everything below it; stops (returning true) as soon as
/// `f` returns true.
pub(crate) fn walk_stmt(stmt: &Stmt, f: &mut dyn FnMut(Node) -> bool) -> bool {
    if f(Node::Stmt(stmt)) {
        return true;
    }
    match stmt {
        Stmt::Let { init, .. } => match init {
            LetInit::Assign(e) | LetInit::Tint(e) => walk_expr(e, f),
        },
        Stmt::Assign { lhs, rhs, .. } => walk_expr(lhs, f) || walk_expr(rhs, f),
        Stmt::CompoundAssign { expr, .. } => walk_expr(expr, f),
        Stmt::Expr(e) => walk_expr(e, f),
        Stmt::If { cond, then, else_, .. } => {
            walk_expr(cond, f)
                || walk_block(then, f)
                || else_.as_ref().is_some_and(|b| walk_block(b, f))
        }
        Stmt::While { cond, body, .. } => walk_expr(cond, f) || walk_block(body, f),
        Stmt::Loop { body, .. } => walk_block(body, f),
        Stmt::For { start, end, body, .. } => {
            walk_expr(start, f) || walk_expr(end, f) || walk_block(body, f)
        }
        Stmt::ForIn { iter, body, .. } => walk_expr(iter, f) || walk_block(body, f),
        Stmt::Match { expr, arms, .. } => walk_expr(expr, f) || walk_arms(arms, f),
        Stmt::Return(e, _) => walk_expr(e, f),
        Stmt::Break(_) | Stmt::Continue(_) => false,
    }
}

fn walk_arms(arms: &[MatchArm], f: &mut dyn FnMut(Node) -> bool) -> bool {
    arms.iter().any(|arm| {
        arm.guard.as_ref().is_some_and(|g| walk_expr(g, f)) || walk_expr(&arm.expr, f)
    })
}

fn walk_fields(fields: &[StructInitField], f: &mut dyn FnMut(Node) -> bool) -> bool {
    fields.iter().any(|field| match field {
        StructInitField::Assign { expr, .. } | StructInitField::Tint { expr, .. } => walk_expr(expr, f),
    })
}

pub(crate) fn walk_expr(expr: &Expr, f: &mut dyn FnMut(Node) -> bool) -> bool {
    if f(Node::Expr(expr)) {
        return true;
    }
    match expr {
        Expr::Number(..) | Expr::String(..) | Expr::Bool(..) | Expr::Unit(_) | Expr::Ident(..) | Expr::SelfKw(_) => false,
        Expr::InterpolatedString { parts, .. } => parts.iter().any(|p| match p {
            StringPart::Expr(e) => walk_expr(e, f),
            StringPart::Text(_) => false,
        }),
        Expr::Call { target, args, .. } => walk_expr(target, f) || args.iter().any(|a| walk_expr(a, f)),
        Expr::Field { target, .. } | Expr::TupleIndex { target, .. } => walk_expr(target, f),
        Expr::Array { items, .. } | Expr::Tuple { items, .. } => items.iter().any(|a| walk_expr(a, f)),
        Expr::Namespace { base, .. } => walk_expr(base, f),
        Expr::Index { target, index, .. } => walk_expr(target, f) || walk_expr(index, f),
        Expr::Unary { expr, .. } | Expr::Paren(expr, _) | Expr::Try { expr, .. } | Expr::Cast { expr, .. } => {
            walk_expr(expr, f)
        }
        Expr::Binary { left, right, .. } => walk_expr(left, f) || walk_expr(right, f),
        Expr::Match { scrutinee, arms, .. } => walk_expr(scrutinee, f) || walk_arms(arms, f),
        Expr::If { cond, then, else_, .. } => {
            walk_expr(cond, f) || walk_block(then, f) || walk_block(else_, f)
        }
        Expr::Lambda { body, .. } => walk_expr(body, f),
        Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => walk_fields(fields, f),
        Expr::StructUpdate { base, updates, .. } => walk_expr(base, f) || walk_fields(updates, f),
        Expr::NamedArg { value, .. } => walk_expr(value, f),
        Expr::Block(block, _) => walk_block(block, f),
        Expr::MapInit { entries, .. } => entries.iter().any(|(_, e)| walk_expr(e, f)),
        Expr::Borrow { target, block, .. } => {
            walk_expr(target, f) || block.as_ref().is_some_and(|b| walk_block(b, f))
        }
    }
}

/// The variable at the bottom of a place expression (`a.b[i].c` -> `a`).
pub(crate) fn place_root(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name),
        Expr::SelfKw(_) => Some("self"),
        Expr::Field { target, .. } | Expr::Index { target, .. } | Expr::TupleIndex { target, .. } => {
            place_root(target)
        }
        Expr::Paren(inner, _) => place_root(inner),
        _ => None,
    }
}

pub(crate) fn is_self_param(p: &tint_ast::Param) -> bool {
    matches!(strip_pattern(&p.pattern), Pattern::Ident(name, _) if name == "self")
}

/// Names of the built-in methods that change their receiver.
pub(crate) fn builtin_mutator(receiver: &Type, method: &str) -> bool {
    match receiver {
        Type::Array(_) => matches!(method, "push" | "pop" | "remove"),
        Type::Map(_) => matches!(method, "set" | "remove"),
        _ => false,
    }
}

impl<'a> Lowerer<'a> {
    /// Methods that change `self`: they assign into it or call something that
    /// does. Those return `(result, self)` so the caller can write the new
    /// receiver back (the language passes values, but a method call updates
    /// the variable it was called on).
    pub fn mutating_methods(&self, items: &[&Item]) -> HashSet<(String, String)> {
        let mut methods: Vec<(String, &FnDecl)> = Vec::new();
        for item in items {
            if let Item::Impl(block) = item {
                if let tint_ast::Type::Simple(owner) = &block.target {
                    for m in &block.methods {
                        methods.push((owner.clone(), m));
                    }
                }
            }
        }
        let mut set: HashSet<(String, String)> = HashSet::new();
        loop {
            let mut changed = false;
            for (owner, m) in &methods {
                let key = (owner.clone(), m.name.clone());
                if set.contains(&key) || !m.params.first().is_some_and(is_self_param) {
                    continue;
                }
                if self.mutates_self(m, &set) {
                    set.insert(key);
                    changed = true;
                }
            }
            if !changed {
                return set;
            }
        }
    }

    fn mutates_self(&self, m: &FnDecl, known: &HashSet<(String, String)>) -> bool {
        let mut visit = |node: Node| -> bool {
            match node {
                Node::Stmt(Stmt::Assign { lhs, .. }) => place_root(lhs) == Some("self"),
                Node::Stmt(Stmt::CompoundAssign { name, .. }) => name == "self",
                Node::Expr(Expr::Call { target, .. }) => {
                    let Expr::Field { target: recv, field, .. } = target.as_ref() else {
                        return false;
                    };
                    if place_root(recv) != Some("self") {
                        return false;
                    }
                    match self.model.type_of(recv) {
                        Some(ty @ (Type::Array(_) | Type::Map(_))) => builtin_mutator(ty, field),
                        Some(Type::Struct(name)) => known.contains(&(name.clone(), field.clone())),
                        _ => false,
                    }
                }
                _ => false,
            }
        };
        match &m.body {
            FnBody::Block(b) => walk_block(b, &mut visit),
            FnBody::Expr(e) => walk_expr(e, &mut visit),
        }
    }
}
