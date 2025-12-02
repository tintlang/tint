use rune_ast::*;
use crate::{
    errors::{SemanticError, SemanticErrorKind},
    scope::{ScopeStack},
    type_table::{Type, TypeTable},
    prelude::{CheckerContext, Mode},
};

pub struct SemanticChecker {
    pub errors: Vec<SemanticError>,
    scopes: ScopeStack,
    ctx: CheckerContext,
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            errors: vec![],
            scopes: ScopeStack::new(),
            ctx: CheckerContext::default(),
        }
    }

    pub fn check(&mut self, program: &Program) {
        for item in &program.items {
            self.visit_item(item);
        }
    }

    // TOP LEVEL ITEMS
    fn visit_item(&mut self, item: &Item) {
        match item {
            Item::Fn(f) => self.visit_fn(f),
            Item::UiFn(u) => self.visit_ui_fn(u),
            Item::Struct(_) => {}
            Item::Enum(_) => {}
            _ => {}
        }
    }

    // Logic Function
    fn visit_fn(&mut self, f: &FnDecl) {
        self.ctx.mode = Mode::Logic;
        self.scopes.push();
        self.visit_block(&f.body);
        self.scopes.pop();
    }

    // UI Function
    fn visit_ui_fn(&mut self, f: &UiFnDecl) {
        self.ctx.mode = Mode::Logic; // parameters = logic
        self.scopes.push();
        // TODO: bind params

        self.ctx.mode = Mode::UI;
        self.visit_ui_block(&f.body);

        self.scopes.pop();
    }

    // UI Block
    fn visit_ui_block(&mut self, block: &UiBlock) {
        for child in &block.nodes {
            self.visit_ui_node(child);
        }
    }

    fn visit_ui_node(&mut self, node: &UiNode) {
        match node {
            UiNode::Element(el) => self.visit_element(el),
            UiNode::Text(_) => {}
            UiNode::Block(b) => self.visit_block_if(b),
        }
    }

    fn visit_element(&mut self, el: &UiElement) {
        for attr in &el.attributes {
            self.visit_attribute(attr);
        }

        if let Some(children) = &el.children {
            for c in children {
                self.visit_ui_node(c);
            }
        }
    }

    // Attributes
    fn visit_attribute(&mut self, attr: &UiAttribute) {
        match attr {
            UiAttribute::LogicAssign { span, .. } => {
                match self.ctx.mode {
                    Mode::UI => self.error(*span, SemanticErrorKind::InvalidLogicInUi),
                    Mode::Logic => {}
                }
            }

            UiAttribute::Modifier { span, .. } => {
                match self.ctx.mode {
                    Mode::Logic => self.error(*span, SemanticErrorKind::InvalidUiInLogic),
                    Mode::UI => {}
                }
            }
        }
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
            Stmt::Let(l) => self.visit_let(l),
            Stmt::Assign(a) => self.visit_assign(a),
            Stmt::Expr(e) => { self.visit_expr(e); }
            Stmt::If(i) => {}
            Stmt::For(f) => {}
            Stmt::Match(m) => {}
            _ => {}
        }
    }

    // let / assign
    fn visit_let(&mut self, l: &LetStmt) {
        // TODO type inference
        if !self.scopes.define(&l.name, Type::Unknown) {
            self.error(l.span, SemanticErrorKind::DuplicateIdent(l.name.clone()));
        }
    }

    fn visit_assign(&mut self, a: &AssignStmt) {
        if self.scopes.lookup(&a.name).is_none() {
            self.error(a.span, SemanticErrorKind::UnknownIdent(a.name.clone()));
        }
    }

    // Expression
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(id) => {
                if self.scopes.lookup(id).is_none() {
                    self.error(expr.span(), SemanticErrorKind::UnknownIdent(id.clone()));
                }
            }

            Expr::Binary { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }

            Expr::Literal(_) => {}

            _ => {}
        }
    }

    //
    fn visit_block_if(&mut self, _blk: &UiLogicBlock) {
        // TODO: logic for if/for/match inside UI
    }

    // Utility
    fn error(&mut self, span: Span, kind: SemanticErrorKind) {
        self.errors.push(SemanticError::new(kind, span));
    }
}
