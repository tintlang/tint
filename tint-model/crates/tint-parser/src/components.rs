// Components with props and their own state.
//
//   component Counter(label: string, start: i32 {0}) {
//       state n = start
//       fn inc() { n = n + 1 }
//       padding::8
//       Button { click||inc "{label}: {n}" }
//   }
//   Counter { label::"Clicks" start::5 }
//
// Every use of such a component is expanded here, right after parsing, into the plain
// element a component without props would give, so the checker, the interpreter and the
// compiled backend never see props or component state:
//   - a prop is replaced by the expression given at the use (or its default), everywhere
//     in the component, like a macro argument;
//   - `state` and `fn` of the component become the `ui fn`'s own, renamed per use
//     (`Counter__1__n`), so every use has its own state and handlers.
// Components without params, state or fns are left to the existing template mechanism.

use std::collections::{HashMap, HashSet};

use crate::error::{PResult, ParserError};
use tint_ast::*;

const MAX_DEPTH: usize = 24;

struct Def {
    params: Vec<Param>,
    state: Vec<UiStateDecl>,
    fns: Vec<FnDecl>,
    attributes: Vec<UiAttribute>,
    modifiers: Vec<UiModifier>,
    children: Vec<UiNodeOrExpr>,
    variants: HashMap<String, (Vec<UiModifier>, Vec<UiNodeOrExpr>)>,
}

impl Def {
    fn is_stateful(&self) -> bool {
        !self.state.is_empty() || !self.fns.is_empty()
    }
}

pub fn expand_components(program: &mut Program) -> PResult<()> {
    let mut hoisted: Vec<Item> = Vec::new();
    for item in &mut program.items {
        if let Item::UiFn(f) = item {
            expand_ui_fn(f, &mut hoisted)?;
        }
    }
    program.items.extend(hoisted);
    expand_persist(program);
    Ok(())
}

// `persist state name = "x"`: a string state saved in storage. Its initial value is
// `storage_get_or("name", "x")`, and every assignment to it in a handler is followed by
// `storage_set("name", name)`.
fn expand_persist(program: &mut Program) {
    let mut names: HashSet<String> = HashSet::new();
    for item in &mut program.items {
        if let Item::UiFn(f) = item {
            for s in f.state.iter_mut().filter(|s| s.persist) {
                let span = s.span;
                let key = Expr::String(s.name.clone(), span);
                let default = s.init.clone();
                s.init = Expr::Call {
                    target: Box::new(Expr::Ident("storage_get_or".into(), span)),
                    args: vec![key, default],
                    span,
                };
                names.insert(s.name.clone());
            }
        }
    }
    if names.is_empty() {
        return;
    }
    for item in &mut program.items {
        if let Item::Fn(f) | Item::ExportFn(f, _) = item {
            if let FnBody::Block(b) = &mut f.body {
                persist_block(b, &names);
            }
        }
    }
}

fn persist_block(b: &mut Block, names: &HashSet<String>) {
    let stmts = std::mem::take(&mut b.stmts);
    for mut stmt in stmts {
        let target = match &stmt {
            Stmt::Assign { lhs: Expr::Ident(n, _), span, .. } if names.contains(n) => Some((n.clone(), *span)),
            Stmt::CompoundAssign { name, span, .. } if names.contains(name) => Some((name.clone(), *span)),
            _ => None,
        };
        match &mut stmt {
            Stmt::If { then, else_, .. } => {
                persist_block(then, names);
                if let Some(e) = else_ {
                    persist_block(e, names);
                }
            }
            Stmt::While { body, .. } | Stmt::Loop { body, .. } | Stmt::For { body, .. } | Stmt::ForIn { body, .. } => {
                persist_block(body, names)
            }
            _ => {}
        }
        b.stmts.push(stmt);
        if let Some((name, span)) = target {
            b.stmts.push(Stmt::Expr(Expr::Call {
                target: Box::new(Expr::Ident("storage_set".into(), span)),
                args: vec![Expr::String(name.clone(), span), Expr::Ident(name, span)],
                span,
            }));
        }
    }
}

fn err<T>(span: Span, msg: String) -> PResult<T> {
    Err(ParserError::Message { msg, span })
}

fn expand_ui_fn(f: &mut UiFnDecl, hoisted: &mut Vec<Item>) -> PResult<()> {
    // `derived name = expr` at the top of a `ui fn`: the expression stands for the name in the view.
    if f.state.iter().any(|s| s.derived) {
        let mut subst = Subst::default();
        for s in f.state.iter().filter(|s| s.derived) {
            let mut value = s.init.clone();
            subst.expr(&mut value);
            subst.set(&s.name, Repl::Expr(value));
        }
        f.state.retain(|s| !s.derived);
        for node in &mut f.body {
            subst.node(node);
        }
    }
    let mut defs: HashMap<String, Def> = HashMap::new();
    let mut body = std::mem::take(&mut f.body);
    take_defs_nodes(&mut body, &mut defs);
    if defs.is_empty() {
        f.body = body;
        return Ok(());
    }
    let mut ctx = Ctx { defs, counters: HashMap::new(), new_state: Vec::new(), hoisted: Vec::new() };
    let mut out = Vec::new();
    for node in body {
        out.extend(ctx.expand_node(node, false, 0)?);
    }
    f.body = out;
    f.state.extend(ctx.new_state);
    hoisted.extend(ctx.hoisted);
    Ok(())
}

// ---- finding declarations ------------------------------------------------------------

fn is_rich(params: &[Param], state: &[UiStateDecl], fns: &[FnDecl]) -> bool {
    !params.is_empty() || !state.is_empty() || !fns.is_empty()
}

/// Removes the declarations of rich components from the tree and records them.
fn take_defs_nodes(nodes: &mut Vec<UiNode>, defs: &mut HashMap<String, Def>) {
    let old = std::mem::take(nodes);
    for node in old {
        if let Some(node) = take_defs_node(node, defs) {
            nodes.push(node);
        }
    }
}

fn take_defs_children(children: &mut Vec<UiNodeOrExpr>, defs: &mut HashMap<String, Def>) {
    let old = std::mem::take(children);
    for child in old {
        match child {
            UiNodeOrExpr::Node(node) => {
                if let Some(node) = take_defs_node(node, defs) {
                    children.push(UiNodeOrExpr::Node(node));
                }
            }
            text => children.push(text),
        }
    }
}

fn take_defs_node(node: UiNode, defs: &mut HashMap<String, Def>) -> Option<UiNode> {
    match node {
        UiNode::Component { name, params, state, fns, attributes, modifiers, mut children, span } => {
            take_defs_children(&mut children, defs);
            if is_rich(&params, &state, &fns) || !attributes.is_empty() {
                let mut variants = HashMap::new();
                let mut template = Vec::new();
                for child in children {
                    match child {
                        UiNodeOrExpr::Node(UiNode::Variant { name, modifiers, children, .. }) => {
                            variants.insert(name, (modifiers, children));
                        }
                        other => template.push(other),
                    }
                }
                defs.insert(name, Def { params, state, fns, attributes, modifiers, children: template, variants });
                None
            } else {
                Some(UiNode::Component { name, params, state, fns, attributes, modifiers, children, span })
            }
        }
        UiNode::BlockElement { name, attributes, modifiers, mut children, span } => {
            take_defs_children(&mut children, defs);
            Some(UiNode::BlockElement { name, attributes, modifiers, children, span })
        }
        UiNode::Theme { name, mut children, span } => {
            take_defs_children(&mut children, defs);
            Some(UiNode::Theme { name, children, span })
        }
        UiNode::For { var, iterable, mut body, span } => {
            take_defs_children(&mut body, defs);
            Some(UiNode::For { var, iterable, body, span })
        }
        other => Some(other),
    }
}

// ---- expansion -----------------------------------------------------------------------

struct Ctx {
    defs: HashMap<String, Def>,
    counters: HashMap<String, usize>,
    new_state: Vec<UiStateDecl>,
    hoisted: Vec<Item>,
}

impl Ctx {
    fn expand_children(&mut self, children: Vec<UiNodeOrExpr>, in_loop: bool, depth: usize) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();
        for child in children {
            match child {
                UiNodeOrExpr::Node(node) => {
                    for n in self.expand_node(node, in_loop, depth)? {
                        out.push(UiNodeOrExpr::Node(n));
                    }
                }
                text => out.push(text),
            }
        }
        Ok(out)
    }

    fn expand_node(&mut self, node: UiNode, in_loop: bool, depth: usize) -> PResult<Vec<UiNode>> {
        if depth > MAX_DEPTH {
            return err(node.span(), "components use each other without end".to_string());
        }
        match node {
            UiNode::BlockElement { name, attributes, modifiers, children, span } => {
                let looped = in_loop || modifiers.iter().any(|m| m.path == ["for".to_string()]);
                if self.defs.contains_key(&name) {
                    let children = self.expand_children(children, looped, depth)?;
                    let node = self.instantiate(&name, attributes, modifiers, children, looped, span, depth)?;
                    return Ok(vec![node]);
                }
                let children = self.expand_children(children, looped, depth)?;
                Ok(vec![UiNode::BlockElement { name, attributes, modifiers, children, span }])
            }
            UiNode::BlockSelfClosing { name, modifiers, span } => {
                if self.defs.contains_key(&name) {
                    let looped = in_loop || modifiers.iter().any(|m| m.path == ["for".to_string()]);
                    let node = self.instantiate(&name, Vec::new(), modifiers, Vec::new(), looped, span, depth)?;
                    return Ok(vec![node]);
                }
                Ok(vec![UiNode::BlockSelfClosing { name, modifiers, span }])
            }
            UiNode::Theme { name, children, span } => {
                let children = self.expand_children(children, in_loop, depth)?;
                Ok(vec![UiNode::Theme { name, children, span }])
            }
            UiNode::For { var, iterable, body, span } => {
                let body = self.expand_children(body, true, depth)?;
                Ok(vec![UiNode::For { var, iterable, body, span }])
            }
            UiNode::Component { name, params, state, fns, attributes, modifiers, children, span } => {
                let children = self.expand_children(children, in_loop, depth)?;
                Ok(vec![UiNode::Component { name, params, state, fns, attributes, modifiers, children, span }])
            }
            UiNode::Slot { name, children, span } => {
                let children = self.expand_children(children, in_loop, depth)?;
                Ok(vec![UiNode::Slot { name, children, span }])
            }
            other => Ok(vec![other]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiate(
        &mut self,
        name: &str,
        attributes: Vec<UiAttribute>,
        call_modifiers: Vec<UiModifier>,
        call_children: Vec<UiNodeOrExpr>,
        in_loop: bool,
        span: Span,
        depth: usize,
    ) -> PResult<UiNode> {
        let def = &self.defs[name];
        if def.is_stateful() && in_loop {
            return err(
                span,
                format!("`{name}` has state, so it cannot be used inside a `for`: every row would share it"),
            );
        }
        let k = {
            let c = self.counters.entry(name.to_string()).or_insert(0);
            *c += 1;
            *c
        };

        // Props: from the use, else the default.
        let mut subst = Subst::default();
        let mut forwarded: Vec<UiModifier> = Vec::new();
        let mut given: HashMap<String, UiModifierValue> = HashMap::new();
        let prop_names: HashSet<String> = def.params.iter().filter_map(|p| param_name(p).map(str::to_string)).collect();
        let mut variant_name: Option<String> = None;
        for m in call_modifiers {
            if m.path.len() == 1 && prop_names.contains(&m.path[0]) {
                given.insert(m.path[0].clone(), m.value);
            } else if m.path == ["variant".to_string()] {
                if let UiModifierValue::Ident(v) = &m.value {
                    variant_name = Some(v.clone());
                }
            } else {
                forwarded.push(m);
            }
        }
        for p in &def.params {
            let Some(pname) = param_name(p) else { continue };
            let value = match (given.remove(pname), &p.default) {
                (Some(v), _) => modifier_value_expr(&v, span)?,
                (None, Some(DefaultValue::Single(e) | DefaultValue::Broadcast(e))) => e.clone(),
                _ => return err(span, format!("`{name}` needs the prop `{pname}`: write `{pname}::value`")),
            };
            subst.set(pname, Repl::Expr(value));
        }

        // State and handlers get this use's own names.
        let mangle = |n: &str| format!("{name}__{k}__{n}");
        for s in def.state.iter().filter(|s| !s.derived) {
            subst.set(&s.name, Repl::Name(mangle(&s.name)));
        }
        for f in &def.fns {
            subst.set(&f.name, Repl::Name(mangle(&f.name)));
        }
        // `derived` lines are names for expressions, in order (a later one may use an earlier one).
        for s in def.state.iter().filter(|s| s.derived) {
            let mut value = s.init.clone();
            subst.expr(&mut value);
            subst.set(&s.name, Repl::Expr(value));
        }
        for s in def.state.iter().filter(|s| !s.derived) {
            let mut init = s.init.clone();
            subst.expr(&mut init);
            self.new_state.push(UiStateDecl { name: mangle(&s.name), init, derived: false, persist: s.persist, span: s.span });
        }
        for f in &def.fns {
            let mut decl = f.clone();
            decl.name = mangle(&f.name);
            subst.fn_decl(&mut decl);
            self.hoisted.push(Item::Fn(decl));
        }

        let decl_attributes = def.attributes.clone();
        // The element: component modifiers, variant modifiers, then the use's.
        let mut modifiers = def.modifiers.clone();
        let mut template = def.children.clone();
        if let Some(v) = variant_name {
            if let Some((vm, vc)) = def.variants.get(&v) {
                modifiers.extend(vm.iter().cloned());
                template.extend(vc.iter().cloned());
            }
        }
        for m in &mut modifiers {
            subst.modifier(m);
        }
        for child in &mut template {
            subst.child(child);
        }
        modifiers.extend(forwarded);

        // Other components used inside this one are expanded first, so that the use's
        // own children (already expanded) are not looked at a second time.
        let template = self.expand_children(template, in_loop, depth + 1)?;

        // Slots take the use's children (not renamed: they belong to the caller).
        let mut named: HashMap<String, Vec<UiNodeOrExpr>> = HashMap::new();
        let mut default_children = Vec::new();
        for child in call_children {
            match child {
                UiNodeOrExpr::Node(UiNode::Slot { name, children, .. }) => {
                    named.insert(name, children);
                }
                other => default_children.push(other),
            }
        }
        let children = fill_slots(template, &named, &default_children);
        let mut all_attributes = decl_attributes;
        for a in &mut all_attributes {
            subst.attribute(a);
        }
        all_attributes.extend(attributes);
        Ok(UiNode::BlockElement { name: name.to_string(), attributes: all_attributes, modifiers, children, span })
    }
}

fn param_name(p: &Param) -> Option<&str> {
    match &p.pattern {
        Pattern::Ident(n, _) => Some(n),
        Pattern::Typed { pat, .. } => match pat.as_ref() {
            Pattern::Ident(n, _) => Some(n),
            _ => None,
        },
        Pattern::Mut { inner, .. } => match inner.as_ref() {
            Pattern::Ident(n, _) => Some(n),
            _ => None,
        },
        _ => None,
    }
}

fn modifier_value_expr(v: &UiModifierValue, span: Span) -> PResult<Expr> {
    Ok(match v {
        UiModifierValue::Number(n) => Expr::Number(if n.fract() == 0.0 { format!("{}", *n as i64) } else { n.to_string() }, span),
        UiModifierValue::String(s) => Expr::String(s.clone(), span),
        UiModifierValue::Ident(s) => Expr::String(s.clone(), span),
        UiModifierValue::Expr(e) => e.clone(),
        // `prop::{ name }`: a lone name in braces reads as a one-item tuple.
        UiModifierValue::Tuple(items) if items.len() == 1 => match &items[0] {
            UiModifierValue::Ident(s) => Expr::Ident(s.clone(), span),
            other => return modifier_value_expr(other, span),
        },
        _ => return err(span, "a prop takes a number, a string or an expression in braces".to_string()),
    })
}

fn fill_slots(
    nodes: Vec<UiNodeOrExpr>,
    named: &HashMap<String, Vec<UiNodeOrExpr>>,
    default_children: &[UiNodeOrExpr],
) -> Vec<UiNodeOrExpr> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            UiNodeOrExpr::Node(UiNode::Slot { name, .. }) => match named.get(&name) {
                Some(content) => out.extend(content.iter().cloned()),
                None if name == "content" => out.extend(default_children.iter().cloned()),
                None => {}
            },
            UiNodeOrExpr::Node(UiNode::BlockElement { name, attributes, modifiers, children, span }) => {
                out.push(UiNodeOrExpr::Node(UiNode::BlockElement {
                    name,
                    attributes,
                    modifiers,
                    children: fill_slots(children, named, default_children),
                    span,
                }));
            }
            UiNodeOrExpr::Node(UiNode::For { var, iterable, body, span }) => {
                out.push(UiNodeOrExpr::Node(UiNode::For {
                    var,
                    iterable,
                    body: fill_slots(body, named, default_children),
                    span,
                }));
            }
            other => out.push(other),
        }
    }
    out
}

// ---- substitution ---------------------------------------------------------------------

#[derive(Clone)]
enum Repl {
    /// A prop: the expression written at the use.
    Expr(Expr),
    /// State or a handler of this use: its new name.
    Name(String),
}

#[derive(Default)]
struct Subst {
    map: HashMap<String, Repl>,
    /// Names bound by lambdas, `let`, `for`, patterns: they hide a prop or state of the same name.
    shadow: Vec<HashSet<String>>,
}

impl Subst {
    fn set(&mut self, name: &str, r: Repl) {
        self.map.insert(name.to_string(), r);
    }

    fn hidden(&self, name: &str) -> bool {
        self.shadow.iter().any(|s| s.contains(name))
    }

    fn with_scope<R>(&mut self, names: HashSet<String>, f: impl FnOnce(&mut Self) -> R) -> R {
        self.shadow.push(names);
        let r = f(self);
        self.shadow.pop();
        r
    }

    fn name_of(&self, name: &str) -> Option<&String> {
        match self.map.get(name) {
            Some(Repl::Name(n)) if !self.hidden(name) => Some(n),
            _ => None,
        }
    }

    fn rename(&self, name: &mut String) {
        if let Some(n) = self.name_of(name) {
            *name = n.clone();
        }
    }

    // ---- expressions

    fn expr(&mut self, e: &mut Expr) {
        match e {
            Expr::Ident(name, span) => {
                if self.hidden(name) {
                    return;
                }
                match self.map.get(name.as_str()).cloned() {
                    Some(Repl::Name(n)) => *name = n,
                    Some(Repl::Expr(x)) => *e = Expr::Paren(Box::new(x), *span),
                    None => {}
                }
            }
            Expr::Number(..) | Expr::String(..) | Expr::Bool(..) | Expr::Unit(..) | Expr::SelfKw(..) => {}
            Expr::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let StringPart::Expr(x) = p {
                        self.expr(x);
                    }
                }
            }
            Expr::Call { target, args, .. } => {
                self.expr(target);
                args.iter_mut().for_each(|a| self.expr(a));
            }
            Expr::Field { target, .. } | Expr::TupleIndex { target, .. } => self.expr(target),
            Expr::Array { items, .. } | Expr::Tuple { items, .. } => items.iter_mut().for_each(|a| self.expr(a)),
            Expr::Namespace { base, .. } => self.expr(base),
            Expr::Index { target, index, .. } => {
                self.expr(target);
                self.expr(index);
            }
            Expr::Unary { expr, .. } | Expr::Paren(expr, _) | Expr::Try { expr, .. } | Expr::Cast { expr, .. } => self.expr(expr),
            Expr::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Match { scrutinee, arms, .. } => {
                self.expr(scrutinee);
                self.arms(arms);
            }
            Expr::If { cond, then, else_, .. } => {
                self.expr(cond);
                self.block(then);
                self.block(else_);
            }
            Expr::Lambda { params, body, .. } => {
                let names: HashSet<String> = params.iter().cloned().collect();
                self.with_scope(names, |s| s.expr(body));
            }
            Expr::StructInit { fields, .. } | Expr::VariantInit { fields, .. } => self.fields(fields),
            Expr::StructUpdate { base, updates, .. } => {
                self.expr(base);
                self.fields(updates);
            }
            Expr::NamedArg { value, .. } => self.expr(value),
            Expr::Block(b, _) => self.block(b),
            Expr::MapInit { entries, .. } => entries.iter_mut().for_each(|(_, v)| self.expr(v)),
            Expr::Borrow { target, block, .. } => {
                self.expr(target);
                if let Some(b) = block {
                    self.block(b);
                }
            }
        }
    }

    fn fields(&mut self, fields: &mut [StructInitField]) {
        for f in fields {
            match f {
                StructInitField::Assign { expr, .. } | StructInitField::Tint { expr, .. } => self.expr(expr),
            }
        }
    }

    fn arms(&mut self, arms: &mut [MatchArm]) {
        for arm in arms {
            let mut names = HashSet::new();
            binders(&arm.pattern, &mut names);
            self.with_scope(names, |s| {
                if let Some(g) = &mut arm.guard {
                    s.expr(g);
                }
                s.expr(&mut arm.expr);
            });
        }
    }

    // ---- statements

    fn block(&mut self, b: &mut Block) {
        self.shadow.push(HashSet::new());
        for stmt in &mut b.stmts {
            self.stmt(stmt);
        }
        self.shadow.pop();
    }

    fn stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let { pattern, init, .. } => {
                match init {
                    LetInit::Assign(x) | LetInit::Tint(x) => self.expr(x),
                }
                let mut names = HashSet::new();
                binders(pattern, &mut names);
                if let Some(top) = self.shadow.last_mut() {
                    top.extend(names);
                }
            }
            Stmt::Assign { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Stmt::CompoundAssign { name, expr, .. } => {
                self.rename(name);
                self.expr(expr);
            }
            Stmt::Expr(x) | Stmt::Return(x, _) => self.expr(x),
            Stmt::If { cond, then, else_, .. } => {
                self.expr(cond);
                self.block(then);
                if let Some(e) = else_ {
                    self.block(e);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.expr(cond);
                self.block(body);
            }
            Stmt::Loop { body, .. } => self.block(body),
            Stmt::Break(..) | Stmt::Continue(..) => {}
            Stmt::For { var, start, end, body, .. } => {
                self.expr(start);
                self.expr(end);
                let names = HashSet::from([var.clone()]);
                self.with_scope(names, |s| s.block(body));
            }
            Stmt::ForIn { var, iter, body, .. } => {
                self.expr(iter);
                let names = HashSet::from([var.clone()]);
                self.with_scope(names, |s| s.block(body));
            }
            Stmt::Match { expr, arms, .. } => {
                self.expr(expr);
                self.arms(arms);
            }
        }
    }

    fn fn_decl(&mut self, f: &mut FnDecl) {
        let mut names = HashSet::new();
        for p in &f.params {
            binders(&p.pattern, &mut names);
        }
        self.with_scope(names, |s| match &mut f.body {
            FnBody::Block(b) => s.block(b),
            FnBody::Expr(x) => s.expr(x),
        });
    }

    // ---- UI

    fn modifier_value(&mut self, v: &mut UiModifierValue) {
        match v {
            UiModifierValue::Expr(e) => self.expr(e),
            UiModifierValue::Block(items) => items.iter_mut().for_each(|m| self.modifier(m)),
            UiModifierValue::Tuple(items) => items.iter_mut().for_each(|i| self.modifier_value(i)),
            UiModifierValue::MiniMod { value, .. } => self.modifier_value(value),
            UiModifierValue::Number(_) | UiModifierValue::String(_) | UiModifierValue::Range(..) | UiModifierValue::Ident(_) => {}
        }
    }

    fn modifier(&mut self, m: &mut UiModifier) {
        // `for{ x in xs }` binds `x` for the rest of the node; its iterable is read outside.
        if m.path == ["for".to_string()] {
            if let UiModifierValue::MiniMod { value, .. } = &mut m.value {
                self.modifier_value(value);
                return;
            }
        }
        self.modifier_value(&mut m.value);
    }

    fn attribute(&mut self, a: &mut UiAttribute) {
        match &mut a.value {
            UiAttrValue::Ident(name) => self.rename(name),
            UiAttrValue::Expr(e) => self.expr(e),
            UiAttrValue::Modifier(block) => {
                for item in &mut block.items {
                    self.modifier_item(item);
                }
            }
            UiAttrValue::Literal(_) => {}
        }
    }

    fn modifier_item(&mut self, item: &mut UiModifierItem) {
        if let Some(v) = &mut item.value {
            self.modifier_value(v);
        }
        item.children.iter_mut().for_each(|c| self.modifier_item(c));
    }

    fn child(&mut self, child: &mut UiNodeOrExpr) {
        match child {
            UiNodeOrExpr::Text(t) => {
                for part in &mut t.parts {
                    if let UiTextPart::Interpolation(e, _) = part {
                        self.expr(e);
                    }
                }
            }
            UiNodeOrExpr::Node(n) => self.node(n),
        }
    }

    fn node(&mut self, node: &mut UiNode) {
        match node {
            UiNode::BlockElement { attributes, modifiers, children, .. } => {
                let mut loop_vars = HashSet::new();
                for m in modifiers.iter() {
                    if m.path == ["for".to_string()] {
                        if let UiModifierValue::MiniMod { key, .. } = &m.value {
                            loop_vars.extend(key.iter().cloned());
                        }
                    }
                }
                self.with_scope(loop_vars, |s| {
                    attributes.iter_mut().for_each(|a| s.attribute(a));
                    modifiers.iter_mut().for_each(|m| s.modifier(m));
                    children.iter_mut().for_each(|c| s.child(c));
                });
            }
            UiNode::BlockSelfClosing { modifiers, .. } | UiNode::Style { modifiers, .. } => {
                modifiers.iter_mut().for_each(|m| self.modifier(m));
            }
            UiNode::Theme { children, .. } | UiNode::Slot { children, .. } => children.iter_mut().for_each(|c| self.child(c)),
            UiNode::Component { modifiers, children, .. } | UiNode::Variant { modifiers, children, .. } => {
                modifiers.iter_mut().for_each(|m| self.modifier(m));
                children.iter_mut().for_each(|c| self.child(c));
            }
            UiNode::For { var, iterable, body, .. } => {
                self.expr(iterable);
                let names = HashSet::from([var.clone()]);
                self.with_scope(names, |s| body.iter_mut().for_each(|c| s.child(c)));
            }
        }
    }
}

fn binders(p: &Pattern, out: &mut HashSet<String>) {
    match p {
        Pattern::Ident(n, _) => {
            out.insert(n.clone());
        }
        Pattern::Number(..) | Pattern::String(..) | Pattern::Wildcard(_) => {}
        Pattern::Tuple(items, _) => items.iter().for_each(|i| binders(i, out)),
        Pattern::Struct { fields, .. } | Pattern::Map { fields, .. } | Pattern::Group { fields, .. } => {
            for f in fields {
                match f {
                    PatternField::Shorthand { field, .. } => {
                        out.insert(field.clone());
                    }
                    PatternField::Assign { pat, .. } => binders(pat, out),
                    PatternField::Rest(_) => {}
                }
            }
        }
        Pattern::Variant { args, .. } => args.iter().for_each(|a| binders(a, out)),
        Pattern::Typed { pat, .. } => binders(pat, out),
        Pattern::Mut { inner, .. } => binders(inner, out),
    }
}
