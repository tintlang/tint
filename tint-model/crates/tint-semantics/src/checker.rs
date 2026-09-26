use crate::prelude::{CheckerContext, Mode};
use tint_ast::{Block, Expr, Item, Pattern, Program, Span, Stmt, Type};

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticErrorKind {
    DuplicateIdent(String),
    UnknownIdent(String),
    InvalidUiInLogic,
}

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub kind: SemanticErrorKind,
    pub span: Span,
}

impl SemanticError {
    pub fn new(kind: SemanticErrorKind, span: Span) -> Self {
        SemanticError { kind, span }
    }
}

// Scope management for variable tracking
struct Scopes {
    scopes: Vec<std::collections::HashMap<String, Type>>,
}

impl Scopes {
    fn new() -> Self {
        Scopes {
            scopes: vec![std::collections::HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(std::collections::HashMap::new());
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

pub struct SemanticChecker {
    ctx: CheckerContext,
    scopes: Scopes,
    errors: Vec<SemanticError>,
}

impl SemanticChecker {
    pub fn new(ctx: CheckerContext) -> Self {
        SemanticChecker {
            ctx,
            scopes: Scopes::new(),
            errors: vec![],
        }
    }

    pub fn check(&mut self, program: &Program) -> Vec<SemanticError> {
        for item in &program.items {
            self.visit_item(item);
        }
        self.errors.clone()
    }

    fn visit_item(&mut self, _item: &Item) {
        // TODO: implement semantic checking for items
        // For now, just a placeholder to make the code compile
    }

    // Block (logic)
    fn visit_block(&mut self, block: &Block) {
        self.ctx.mode = Mode::Logic;
        self.scopes.push();
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.scopes.pop();
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                pattern,
                ty: _,
                init: _,
                span,
            } => {
                self.visit_let_pattern(pattern, *span);
            }
            Stmt::Assign { lhs, rhs, span: _ } => {
                self.visit_expr(lhs);
                self.visit_expr(rhs);
            }
            Stmt::CompoundAssign {
                name,
                op: _,
                expr,
                span,
            } => {
                if self.scopes.lookup(name).is_none() {
                    self.error(*span, SemanticErrorKind::UnknownIdent(name.clone()));
                }
                self.visit_expr(expr);
            }
            Stmt::Expr(e) => {
                self.visit_expr(e);
            }
            Stmt::If {
                cond,
                then: _,
                else_: _,
                span: _,
            } => {
                self.visit_expr(cond);
            }
            Stmt::For {
                var,
                start,
                end,
                body: _,
                span,
            } => {
                if !self.scopes.define(var, Type::Simple("i32".to_string())) {
                    self.error(*span, SemanticErrorKind::DuplicateIdent(var.clone()));
                }
                self.visit_expr(start);
                self.visit_expr(end);
            }
            Stmt::Match {
                expr,
                arms: _,
                span: _,
            } => {
                self.visit_expr(expr);
            }
            _ => {}
        }
    }

    // let pattern processing
    fn visit_let_pattern(&mut self, pattern: &Pattern, span: Span) {
        match pattern {
            Pattern::Ident(name, _) => {
                if !self.scopes.define(name, Type::Unit) {
                    self.error(span, SemanticErrorKind::DuplicateIdent(name.clone()));
                }
            }
            _ => {}
        }
    }

    // Expression
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(id, span) => {
                if self.scopes.lookup(id).is_none() {
                    self.error(*span, SemanticErrorKind::UnknownIdent(id.clone()));
                }
            }

            Expr::Number(_, _) | Expr::String(_, _) | Expr::Bool(_, _) | Expr::Unit(_) => {}

            Expr::Call {
                target,
                args,
                span: _,
            } => {
                self.visit_expr(target);
                for arg in args {
                    self.visit_expr(arg);
                }
            }

            Expr::Array { items, span: _ } => {
                for item in items {
                    self.visit_expr(item);
                }
            }

            Expr::Unary {
                op: _,
                expr,
                span: _,
            } => {
                self.visit_expr(expr);
            }

            Expr::Binary {
                left,
                right,
                op: _,
                span: _,
            } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }

            Expr::Field {
                target,
                field: _,
                span: _,
            } => {
                self.visit_expr(target);
            }

            Expr::Index {
                target,
                index,
                span: _,
            } => {
                self.visit_expr(target);
                self.visit_expr(index);
            }

            Expr::InterpolatedString { parts: _, span: _ } => {
                // TODO: check interpolated parts
            }

            Expr::Namespace {
                base: _,
                item: _,
                span: _,
            } => {
                // TODO: check namespace resolution
            }

            Expr::SelfKw(_) => {
                // Self keyword - valid in methods
            }

            _ => {}
        }
    }

    // Utility
    fn error(&mut self, span: Span, kind: SemanticErrorKind) {
        self.errors.push(SemanticError::new(kind, span));
    }
}
