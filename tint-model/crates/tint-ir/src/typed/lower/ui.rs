//! `ui fn` lowering: the tree a ui fn describes becomes `UiOpen`/`UiText`/
//! `UiClose` instructions (see `ui.rs` next to `ir.rs`). Styles, components,
//! variants and slots are expanded here, statically; `if{}`, `for{}`,
//! `match{}`, themes and interpolations are ordinary control flow and
//! expressions around the emitted instructions. The order of evaluation and
//! the resulting tree follow the tree-walking builder in `tint-runtime`.

use super::*;
use crate::typed::ui::{modifier_exprs_of, UiElementTemplate, UiSlot, UiTemplate};
use tint_ast::{
    Expr, UiAttrValue, UiAttribute, UiFnDecl, UiModifier, UiModifierValue, UiNode, UiNodeOrExpr,
    UiStateDecl,
};

const MAX_COMPONENT_DEPTH: usize = 64;

struct Component<'a> {
    modifiers: &'a [UiModifier],
    children: Vec<&'a UiNodeOrExpr>,
    variants: HashMap<String, (&'a [UiModifier], &'a [UiNodeOrExpr])>,
}

/// Style and component declarations of one ui fn.
#[derive(Default)]
struct UiStatic<'a> {
    styles: HashMap<String, &'a [UiModifier]>,
    components: HashMap<String, Component<'a>>,
}

fn node_children(children: &[UiNodeOrExpr]) -> impl Iterator<Item = &UiNode> {
    children.iter().filter_map(|c| match c {
        UiNodeOrExpr::Node(n) => Some(n),
        UiNodeOrExpr::Text(_) => None,
    })
}

impl<'a> UiStatic<'a> {
    fn collect_styles(&mut self, nodes: impl Iterator<Item = &'a UiNode>) {
        for node in nodes {
            match node {
                UiNode::Style { name, modifiers, .. } => {
                    self.styles.insert(name.clone(), modifiers);
                }
                UiNode::Component { children, .. }
                | UiNode::Theme { children, .. }
                | UiNode::BlockElement { children, .. }
                | UiNode::Slot { children, .. }
                | UiNode::Variant { children, .. } => self.collect_styles(node_children(children)),
                UiNode::For { body, .. } => self.collect_styles(node_children(body)),
                UiNode::BlockSelfClosing { .. } => {}
            }
        }
    }

    fn collect_components(&mut self, nodes: impl Iterator<Item = &'a UiNode>) {
        for node in nodes {
            match node {
                UiNode::Component { name, modifiers, children, .. } => {
                    let mut base = Vec::new();
                    let mut variants = HashMap::new();
                    for child in children {
                        match child {
                            UiNodeOrExpr::Node(UiNode::Variant { name, modifiers, children, .. }) => {
                                variants.insert(name.clone(), (modifiers.as_slice(), children.as_slice()));
                            }
                            other => base.push(other),
                        }
                    }
                    self.components
                        .insert(name.clone(), Component { modifiers, children: base, variants });
                    self.collect_components(node_children(children));
                }
                UiNode::Theme { children, .. }
                | UiNode::BlockElement { children, .. }
                | UiNode::Slot { children, .. } => self.collect_components(node_children(children)),
                UiNode::For { body, .. } => self.collect_components(node_children(body)),
                UiNode::Style { .. } | UiNode::Variant { .. } | UiNode::BlockSelfClosing { .. } => {}
            }
        }
    }

    /// `use::Style` replaced by the style's modifiers, recursively.
    fn expand(&self, modifiers: &[&'a UiModifier]) -> Vec<&'a UiModifier> {
        let mut out = Vec::new();
        self.expand_into(modifiers, &mut out, &mut Vec::new());
        out
    }

    fn expand_into(&self, modifiers: &[&'a UiModifier], out: &mut Vec<&'a UiModifier>, stack: &mut Vec<String>) {
        for &modifier in modifiers {
            if modifier.path == ["use".to_string()] {
                if let UiModifierValue::Ident(name) = &modifier.value {
                    if stack.contains(name) {
                        continue;
                    }
                    if let Some(style) = self.styles.get(name) {
                        stack.push(name.clone());
                        let inner: Vec<&UiModifier> = style.iter().collect();
                        self.expand_into(&inner, out, stack);
                        stack.pop();
                    }
                }
            } else {
                out.push(modifier);
            }
        }
    }
}

fn find_if<'m>(modifiers: &[&'m UiModifier]) -> Option<&'m Expr> {
    modifiers.iter().find_map(|m| match (&m.path[..], &m.value) {
        ([p], UiModifierValue::Expr(e)) if p == "if" => Some(e),
        _ => None,
    })
}

fn find_for<'m>(modifiers: &[&'m UiModifier]) -> Option<(&'m str, &'m Expr)> {
    modifiers.iter().find_map(|m| match (&m.path[..], &m.value) {
        ([p], UiModifierValue::MiniMod { key, value }) if p == "for" => match value.as_ref() {
            UiModifierValue::Expr(e) => Some((key.first()?.as_str(), e)),
            _ => None,
        },
        _ => None,
    })
}

fn find_match<'m>(modifiers: &[&'m UiModifier]) -> Option<&'m Expr> {
    modifiers.iter().find_map(|m| match (&m.path[..], &m.value) {
        ([p], UiModifierValue::Expr(e)) if p == "match" => Some(e),
        _ => None,
    })
}

fn case_label(attributes: &[UiAttribute]) -> Option<&str> {
    attributes.iter().find_map(|a| match (&a.name[..], &a.value) {
        ("case", UiAttrValue::Ident(label)) => Some(label.as_str()),
        _ => None,
    })
}

fn refs(items: &[UiNodeOrExpr]) -> Vec<&UiNodeOrExpr> {
    items.iter().collect()
}

impl<'a> Lowerer<'a> {
    /// Declares the globals a ui fn's handlers and body share: its `state`
    /// variables and the two the host provides.
    pub(super) fn declare_ui_globals(&mut self, f: &UiFnDecl) -> LResult<()> {
        for name in f.state.iter().map(|s| s.name.as_str()).chain(["theme", "viewport_width", "route_path"]) {
            if !self.globals.contains_key(name) {
                self.declare_global(name)?;
            }
        }
        Ok(())
    }

    /// Initial values of the globals `declare_ui_globals` made.
    pub(super) fn lower_ui_state_inits(&mut self, ui_fns: &[&'a UiFnDecl]) -> LResult<()> {
        let mut states: Vec<&UiStateDecl> = Vec::new();
        for f in ui_fns {
            states.extend(f.state.iter());
        }
        for state in &states {
            let Some(&global) = self.globals.get(&state.name) else {
                return self.err(None, format!("state `{}` has no known type", state.name));
            };
            let ty = self.module.globals[global.0 as usize].ty;
            let value = self.expr_as(&state.init, ty)?;
            self.emit(Instr::GlobalSet { global, src: value });
        }
        // The host's defaults (see `UiSession`).
        if !ui_fns.is_empty() {
            let width = self.globals["viewport_width"];
            let num = self.module.globals[width.0 as usize].ty;
            let v = self.const_reg(num, Const::Float(1440.0));
            self.emit(Instr::GlobalSet { global: width, src: v });
            if !states.iter().any(|s| s.name == "route_path") {
                let route = self.globals["route_path"];
                let ty = self.module.globals[route.0 as usize].ty;
                let v = self.const_reg(ty, Const::Str("/".into()));
                self.emit(Instr::GlobalSet { global: route, src: v });
            }
            if !states.iter().any(|s| s.name == "theme") {
                let theme = self.globals["theme"];
                let ty = self.module.globals[theme.0 as usize].ty;
                let v = self.const_reg(ty, Const::Str("dark".into()));
                self.emit(Instr::GlobalSet { global: theme, src: v });
            }
        }
        Ok(())
    }

    /// Functions a host calls: `tint:set_<name>(value)` for the variables the
    /// host provides, and `tint:enter:<ui fn>()`, which resets that ui fn's
    /// `state` (what showing a page does).
    pub(super) fn lower_ui_support(&mut self, ui_fns: &[&'a UiFnDecl]) -> LResult<()> {
        if ui_fns.is_empty() {
            return Ok(());
        }
        let unit = self.module.types.unit();
        for name in ["theme", "viewport_width", "route_path"] {
            let Some(&global) = self.globals.get(name) else { continue };
            let ty = self.module.globals[global.0 as usize].ty;
            let fname = format!("tint:set_{name}");
            let id = self.reserve_func(&fname, FuncKind::Fn, unit);
            self.module.functions.insert(fname.clone(), id);
            self.stack.push(FnB::new(fname, FuncKind::Fn, unit));
            let param = self.new_reg(ty);
            self.f().params.push(param);
            self.emit(Instr::GlobalSet { global, src: param });
            let done = self.unit_reg();
            self.terminate(Term::Return(done));
            let fb = self.stack.pop().unwrap();
            self.module.funcs[id.0 as usize] = fb.finish(unit);
        }
        for f in ui_fns {
            let fname = format!("tint:enter:{}", f.name);
            let id = self.reserve_func(&fname, FuncKind::Fn, unit);
            self.module.functions.insert(fname.clone(), id);
            self.stack.push(FnB::new(fname, FuncKind::Fn, unit));
            for state in &f.state {
                if state.name == "theme" {
                    continue;
                }
                let Some(&global) = self.globals.get(&state.name) else {
                    return self.err(None, format!("state `{}` has no known type", state.name));
                };
                let ty = self.module.globals[global.0 as usize].ty;
                let value = self.expr_as(&state.init, ty)?;
                self.emit(Instr::GlobalSet { global, src: value });
            }
            let done = self.unit_reg();
            self.terminate(Term::Return(done));
            let fb = self.stack.pop().unwrap();
            self.module.funcs[id.0 as usize] = fb.finish(unit);
        }
        Ok(())
    }

    /// Declares the function of a ui fn; its body is lowered by `lower_ui_fn`.
    pub(super) fn declare_ui_fn(&mut self, f: &UiFnDecl) -> LResult<FuncId> {
        let unit = self.module.types.unit();
        let id = self.reserve_func(&f.name, FuncKind::Ui, unit);
        self.module.functions.insert(f.name.clone(), id);
        Ok(id)
    }

    pub(super) fn lower_ui_fn(&mut self, f: &'a UiFnDecl, id: FuncId) -> LResult<()> {
        let unit = self.module.types.unit();
        let sig = self.model.functions.get(&f.name).cloned();
        self.stack.push(FnB::new(f.name.clone(), FuncKind::Ui, unit));
        for (index, param) in f.params.iter().enumerate() {
            let bound = match strip_pattern(&param.pattern) {
                Pattern::Ident(_, span) => self.model.bindings.get(&(span.start.offset, span.end.offset)).cloned(),
                _ => None,
            };
            let ty = match (sig.as_ref().and_then(|(params, _)| params.get(index)), bound, &param.ty) {
                (_, _, Some(t)) => self.ast_ty(t)?,
                (Some(t), _, _) if !matches!(t, Type::Var(_) | Type::Unknown) => self.conv(t)?,
                (_, Some(t), _) => self.conv(&t)?,
                _ => return self.err(Some(f.span), "ui fn parameter without a known type"),
            };
            let reg = self.new_reg(ty);
            self.f().params.push(reg);
            match strip_pattern(&param.pattern) {
                Pattern::Ident(name, _) => self.f().define(name, reg),
                _ => return self.err(Some(f.span), "ui fn parameters must be plain names"),
            }
        }

        let mut cx = UiStatic::default();
        cx.collect_styles(f.body.iter());
        cx.collect_components(f.body.iter());
        for node in &f.body {
            self.ui_node(&cx, node, 0)?;
        }
        let value = self.unit_reg();
        self.terminate(Term::Return(value));
        let fb = self.stack.pop().unwrap();
        self.module.funcs[id.0 as usize] = fb.finish(unit);
        Ok(())
    }

    // ------------------------------------------------------ helpers

    fn ui_if(&mut self, cond: Reg, then: impl FnOnce(&mut Self) -> LResult<()>) -> LResult<()> {
        let (then_b, join) = (self.new_block(), self.new_block());
        self.terminate(Term::Branch { cond, then_: then_b, else_: join });
        self.switch_to(then_b);
        then(self)?;
        self.terminate(Term::Jump(join));
        self.switch_to(join);
        Ok(())
    }

    /// Runs `body` once per element of `value` with `var` bound to it; a value
    /// that is not a list counts as a single element.
    fn ui_iterate(
        &mut self,
        value: Reg,
        var: &str,
        mut body: impl FnMut(&mut Self) -> LResult<()>,
    ) -> LResult<()> {
        let TyKind::List(elem) = self.tk(self.reg_ty(value)) else {
            self.push_scope();
            self.f().define(var, value);
            let r = body(self);
            self.pop_scope();
            return r;
        };
        let (num, bool_ty) = (self.num_ty(NumKind::Num), self.bool_ty());
        let n = self.rt(RtFn::ListLen, vec![value], num);
        let counter = self.const_reg(num, Const::Float(0.0));
        let (head, body_b, exit) = (self.new_block(), self.new_block(), self.new_block());
        self.terminate(Term::Jump(head));
        self.switch_to(head);
        let more = self.new_reg(bool_ty);
        self.emit(Instr::Cmp { dst: more, op: CmpOp::Lt, a: counter, b: n });
        self.terminate(Term::Branch { cond: more, then_: body_b, else_: exit });
        self.switch_to(body_b);
        self.push_scope();
        let item = self.new_reg(elem);
        self.emit(Instr::Get { dst: item, base: value, proj: Proj::Index(counter) });
        self.f().define(var, item);
        let outcome = self.ui_memo_region(&mut body);
        self.pop_scope();
        outcome?;
        let one = self.const_reg(num, Const::Float(1.0));
        let next = self.arith(BinOp::Add, NumKind::Num, num, counter, one);
        self.emit(Instr::Mov { dst: counter, src: next });
        self.terminate(Term::Jump(head));
        self.switch_to(exit);
        Ok(())
    }

    /// Runs `body` (one iteration of a `for{}`), memoizable when it is a pure
    /// function of scalars: the host may answer `UiMemo` with a hit and skip
    /// the body when it holds what the body emitted for the same inputs.
    ///
    /// The inputs are the registers defined before the body that it reads, plus
    /// the globals it reads, all numbers, strings or bools. A body that calls a
    /// function, touches a heap value from outside, writes outside itself or
    /// changes theme tokens is left as it is.
    fn ui_memo_region(&mut self, body: &mut dyn FnMut(&mut Self) -> LResult<()>) -> LResult<()> {
        let check_b = self.new_block();
        let after = self.new_block();
        let body_b = self.new_block();
        let outer_regs = self.f().regs.len() as u32;
        self.terminate(Term::Jump(check_b));
        self.switch_to(body_b);
        body(self)?;
        match self.memo_inputs(body_b.0 as usize, outer_regs) {
            None => {
                self.terminate(Term::Jump(after));
                self.switch_to(check_b);
                self.terminate(Term::Jump(body_b));
            }
            Some((hoisted, inputs)) => {
                self.emit(Instr::UiMemoEnd);
                self.terminate(Term::Jump(after));
                self.switch_to(check_b);
                for ins in hoisted {
                    self.emit(ins);
                }
                let bool_ty = self.bool_ty();
                let hit = self.new_reg(bool_ty);
                let site = self.memo_sites;
                self.memo_sites += 1;
                self.emit(Instr::UiMemo { dst: hit, site, inputs });
                self.terminate(Term::Branch { cond: hit, then_: after, else_: body_b });
            }
        }
        self.switch_to(after);
        Ok(())
    }

    /// What the blocks from `first` on depend on, when that is a handful of
    /// scalars: `(instructions to run first, registers to key on)`.
    ///
    /// Comparisons of outer values and reads of globals are not keyed by their
    /// operands but by their results: a row that compares itself with
    /// `selected` is keyed by "am I the selected one", so changing `selected`
    /// only disturbs the two rows whose answer changed. Those instructions are
    /// copied (they cannot trap) ahead of the `UiMemo`.
    ///
    /// `None` when the blocks are not a pure function of such scalars: they
    /// call a function, touch a heap value from outside, write a register
    /// defined before them or change theme tokens.
    fn memo_inputs(&self, first: usize, outer_regs: u32) -> Option<(Vec<Instr>, Vec<Reg>)> {
        let fb = self.stack.last()?;
        let scalar = |ty: TyId| matches!(self.module.types.kind(ty), TyKind::Num(_) | TyKind::Str | TyKind::Bool);
        let region = &fb.blocks[first..];
        // How often each register is written inside.
        let mut defs: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for (instrs, _) in region {
            for ins in instrs {
                if let Some(d) = ins.dst() {
                    if d.0 < outer_regs {
                        return None;
                    }
                    *defs.entry(d.0).or_default() += 1;
                }
                match ins {
                    Instr::Call { .. }
                    | Instr::CallClosure { .. }
                    | Instr::Closure { .. }
                    | Instr::Host { .. }
                    | Instr::GlobalSet { .. }
                    | Instr::UiTokens { .. } => return None,
                    Instr::Set { base, .. } if base.0 < outer_regs => return None,
                    _ => {}
                }
            }
        }
        let single = |r: Reg| defs.get(&r.0) == Some(&1);
        let mut avail: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut consts: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut hoist: Vec<Instr> = Vec::new();
        let mut root: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let mut frontier: Vec<Reg> = Vec::new();
        // The registers a non-hoisted instruction (or a terminator) reads.
        let need = |r: Reg,
                    avail: &std::collections::HashSet<u32>,
                    consts: &std::collections::HashSet<u32>,
                    root: &std::collections::HashMap<u32, u32>,
                    frontier: &mut Vec<Reg>|
         -> bool {
            // A copy of a value is keyed by the value itself.
            let r = Reg(*root.get(&r.0).unwrap_or(&r.0));
            if consts.contains(&r.0) || frontier.contains(&r) {
                return true;
            }
            if r.0 < outer_regs {
                if !scalar(fb.reg_ty(r)) {
                    return false;
                }
                frontier.push(r);
                return true;
            }
            if avail.contains(&r.0) {
                frontier.push(r);
            }
            true
        };
        for (instrs, term) in region {
            for ins in instrs {
                let hoisted = match ins {
                    Instr::Const { dst, .. } if single(*dst) => {
                        consts.insert(dst.0);
                        true
                    }
                    Instr::GlobalGet { dst, global } => {
                        if !single(*dst) || !scalar(self.module.globals[global.0 as usize].ty) {
                            return None;
                        }
                        true
                    }
                    Instr::Cmp { dst, a, b, .. } if single(*dst) => {
                        let ok = |r: &Reg| {
                            (r.0 < outer_regs && scalar(fb.reg_ty(*r))) || avail.contains(&r.0) || consts.contains(&r.0)
                        };
                        ok(a) && ok(b)
                    }
                    Instr::Mov { dst, src } if single(*dst) => {
                        let ok = (src.0 < outer_regs && scalar(fb.reg_ty(*src)))
                            || avail.contains(&src.0)
                            || consts.contains(&src.0);
                        if ok {
                            let r = *root.get(&src.0).unwrap_or(&src.0);
                            root.insert(dst.0, r);
                            if consts.contains(&r) {
                                consts.insert(dst.0);
                            }
                        }
                        ok
                    }
                    Instr::Not { dst, src } if single(*dst) => {
                        (src.0 < outer_regs && scalar(fb.reg_ty(*src))) || avail.contains(&src.0) || consts.contains(&src.0)
                    }
                    _ => false,
                };
                if hoisted {
                    if let Some(d) = ins.dst() {
                        avail.insert(d.0);
                    }
                    hoist.push(ins.clone());
                    continue;
                }
                if matches!(ins, Instr::GlobalGet { .. }) {
                    return None;
                }
                for r in ins.uses() {
                    if !need(r, &avail, &consts, &root, &mut frontier) {
                        return None;
                    }
                }
            }
            let used = match term {
                Some(Term::Branch { cond, .. }) => Some(*cond),
                Some(Term::Switch { value, .. }) => Some(*value),
                _ => None,
            };
            if let Some(r) = used {
                if !need(r, &avail, &consts, &root, &mut frontier) {
                    return None;
                }
            }
            if matches!(term, Some(Term::Return(_)) | Some(Term::Trap(_))) {
                return None;
            }
        }
        if frontier.len() > 8 {
            return None;
        }
        // Keep only the hoisted instructions the keyed registers need.
        let mut needed: std::collections::HashSet<u32> = frontier.iter().map(|r| r.0).collect();
        let mut keep: Vec<Instr> = Vec::new();
        for ins in hoist.into_iter().rev() {
            if ins.dst().is_some_and(|d| needed.contains(&d.0)) {
                for r in ins.uses() {
                    needed.insert(r.0);
                }
                keep.push(ins);
            }
        }
        keep.reverse();
        Some((keep, frontier))
    }

    fn ui_children(&mut self, cx: &UiStatic<'a>, children: &[&'a UiNodeOrExpr], depth: usize) -> LResult<()> {
        for child in children {
            match child {
                UiNodeOrExpr::Node(n) => self.ui_node(cx, n, depth)?,
                UiNodeOrExpr::Text(text) => {
                    let mut parts = Vec::new();
                    for part in &text.parts {
                        match part {
                            tint_ast::UiTextPart::Literal(s, _) => {
                                let ty = self.str_ty();
                                parts.push(self.const_reg(ty, Const::Str(s.clone())));
                            }
                            tint_ast::UiTextPart::Interpolation(e, _) => {
                                let v = self.expr(e, None)?;
                                parts.push(self.to_str(v));
                            }
                        }
                    }
                    let ty = self.str_ty();
                    let src = if parts.is_empty() {
                        self.const_reg(ty, Const::Str(String::new()))
                    } else {
                        self.rt(RtFn::StrConcat, parts, ty)
                    };
                    self.emit(Instr::UiText { src });
                }
            }
        }
        Ok(())
    }

    // -------------------------------------------------------- nodes

    fn ui_node(&mut self, cx: &UiStatic<'a>, node: &'a UiNode, depth: usize) -> LResult<()> {
        match node {
            UiNode::Theme { name, children, .. } => {
                let theme = self.ident("theme", node.span())?;
                let ty = self.str_ty();
                let want = self.const_reg(ty, Const::Str(name.clone()));
                let bool_ty = self.bool_ty();
                let same = self.new_reg(bool_ty);
                self.emit(Instr::Cmp { dst: same, op: CmpOp::Eq, a: theme, b: want });
                self.ui_if(same, |this| {
                    // The tokens of this theme, then its children.
                    for child in node_children(children) {
                        if let UiNode::BlockElement { name, modifiers, .. } = child {
                            if name == "tokens" {
                                let template = this.module.ui_templates.len() as u32;
                                this.module
                                    .ui_templates
                                    .push(UiTemplate::Tokens { modifiers: modifiers.clone() });
                                this.emit(Instr::UiTokens { template });
                            }
                        }
                    }
                    this.ui_children(cx, &refs(children), depth)
                })
            }
            UiNode::BlockElement { name, .. } if name == "tokens" => Ok(()),
            UiNode::Style { .. } | UiNode::Component { .. } | UiNode::Variant { .. } | UiNode::Slot { .. } => {
                Ok(())
            }
            UiNode::BlockElement { name, children, .. } if name == "children" => {
                self.ui_children(cx, &refs(children), depth)
            }
            UiNode::For { var, iterable, body, .. } => {
                let value = self.expr(iterable, None)?;
                let body = refs(body);
                self.ui_iterate(value, var, |this| this.ui_children(cx, &body, depth))
            }
            UiNode::BlockElement { name, attributes, modifiers, children, .. } => {
                let mods: Vec<&UiModifier> = modifiers.iter().collect();
                self.ui_element(cx, name, attributes, &mods, children, depth)
            }
            UiNode::BlockSelfClosing { name, modifiers, .. } => {
                let mods: Vec<&UiModifier> = modifiers.iter().collect();
                self.ui_element(cx, name, &[], &mods, &[], depth)
            }
        }
    }

    /// An element or a component invocation, guarded by its `if{}`.
    fn ui_element(
        &mut self,
        cx: &UiStatic<'a>,
        name: &str,
        attributes: &'a [UiAttribute],
        modifiers: &[&'a UiModifier],
        children: &'a [UiNodeOrExpr],
        depth: usize,
    ) -> LResult<()> {
        let cond = match find_if(modifiers) {
            Some(e) => {
                let bool_ty = self.bool_ty();
                Some((e, self.expr_as(e, bool_ty)?))
            }
            None => None,
        };
        let build = |this: &mut Self| -> LResult<()> {
            let pre: Vec<(&Expr, Reg)> = cond.into_iter().collect();
            match cx.components.get(name) {
                Some(component) => this.ui_component(cx, name, attributes, modifiers, children, component, depth),
                None => {
                    let expanded = cx.expand(modifiers);
                    this.ui_plain(cx, name, attributes, &expanded, children, pre, depth)
                }
            }
        };
        match cond {
            Some((_, reg)) => self.ui_if(reg, build),
            None => build(self),
        }
    }

    fn ui_component(
        &mut self,
        cx: &UiStatic<'a>,
        name: &str,
        attributes: &'a [UiAttribute],
        invocation: &[&'a UiModifier],
        invocation_children: &'a [UiNodeOrExpr],
        component: &Component<'a>,
        depth: usize,
    ) -> LResult<()> {
        if depth >= MAX_COMPONENT_DEPTH {
            return self.err(None, format!("component `{name}` expands into itself"));
        }
        let variant_name = invocation.iter().find_map(|m| match (&m.path[..], &m.value) {
            ([p], UiModifierValue::Ident(v)) if p == "variant" => Some(v.clone()),
            _ => None,
        });
        let variant = variant_name.as_ref().and_then(|v| component.variants.get(v));

        let mut merged: Vec<&UiModifier> = component.modifiers.iter().collect();
        if let Some((mods, _)) = variant {
            merged.extend(mods.iter());
        }
        merged.extend(invocation.iter().copied().filter(|m| m.path != ["variant".to_string()]));
        let expanded = cx.expand(&merged);

        let mut named: HashMap<&str, &'a [UiNodeOrExpr]> = HashMap::new();
        let mut default_children: Vec<&'a UiNodeOrExpr> = Vec::new();
        for child in invocation_children {
            match child {
                UiNodeOrExpr::Node(UiNode::Slot { name, children, .. }) => {
                    named.insert(name.as_str(), children.as_slice());
                }
                other => default_children.push(other),
            }
        }
        let mut template: Vec<&'a UiNodeOrExpr> = component.children.clone();
        if let Some((_, extra)) = variant {
            template.extend(extra.iter());
        }

        // Own `if{}` was handled by the caller; a component keeps it in its
        // modifier list like any element (see `ui_plain`).
        let pre = Vec::new();
        self.ui_open(name, attributes, &expanded, pre)?;
        for child in template {
            match child {
                UiNodeOrExpr::Node(UiNode::Slot { name, .. }) => {
                    let content: Vec<&'a UiNodeOrExpr> = match named.get(name.as_str()) {
                        Some(c) => refs(c),
                        None if name == "content" => default_children.clone(),
                        None => Vec::new(),
                    };
                    self.ui_children(cx, &content, depth + 1)?;
                }
                other => self.ui_children(cx, &[other], depth + 1)?,
            }
        }
        self.emit(Instr::UiClose);
        Ok(())
    }

    fn ui_plain(
        &mut self,
        cx: &UiStatic<'a>,
        name: &str,
        attributes: &'a [UiAttribute],
        expanded: &[&'a UiModifier],
        children: &'a [UiNodeOrExpr],
        mut pre: Vec<(&'a Expr, Reg)>,
        depth: usize,
    ) -> LResult<()> {
        let for_loop = find_for(expanded).map(|(v, e)| (v, e));
        // The `for{}` iterable and `match{}` scrutinee are evaluated once,
        // before the element opens; the modifier list sees the same value.
        let mut iterable = None;
        let mut scrutinee = None;
        if let Some((_, e)) = for_loop {
            let v = self.expr(e, None)?;
            pre.push((e, v));
            iterable = Some(v);
        } else if let Some(e) = find_match(expanded) {
            let v = self.expr(e, None)?;
            pre.push((e, v));
            scrutinee = Some(v);
        }
        self.ui_open(name, attributes, expanded, pre)?;
        let body = refs(children);
        match (for_loop, iterable, scrutinee) {
            (Some((var, _)), Some(list), _) => {
                self.ui_iterate(list, var, |this| this.ui_children(cx, &body, depth))?;
            }
            (_, _, Some(value)) => self.ui_match_children(cx, value, &body, depth)?,
            _ => self.ui_children(cx, &body, depth)?,
        }
        self.emit(Instr::UiClose);
        Ok(())
    }

    /// `match{}`: only the first `case` child whose label equals the text of
    /// the scrutinee (or is `_`) is built; other children always are.
    fn ui_match_children(
        &mut self,
        cx: &UiStatic<'a>,
        scrutinee: Reg,
        children: &[&'a UiNodeOrExpr],
        depth: usize,
    ) -> LResult<()> {
        let bool_ty = self.bool_ty();
        let str_ty = self.str_ty();
        let text = self.to_str(scrutinee);
        let matched = self.const_reg(bool_ty, Const::Bool(false));
        for child in children {
            let UiNodeOrExpr::Node(node) = child else {
                self.ui_children(cx, &[child], depth)?;
                continue;
            };
            let arm = match node {
                UiNode::BlockElement { attributes, children, .. } => {
                    case_label(attributes).map(|label| (label, children))
                }
                _ => None,
            };
            let Some((label, arm_children)) = arm else {
                self.ui_node(cx, node, depth)?;
                continue;
            };
            let hit = if label == "_" {
                self.const_reg(bool_ty, Const::Bool(true))
            } else {
                let want = self.const_reg(str_ty, Const::Str(label.to_string()));
                let eq = self.new_reg(bool_ty);
                self.emit(Instr::Cmp { dst: eq, op: CmpOp::Eq, a: text, b: want });
                eq
            };
            let not_yet = self.new_reg(bool_ty);
            self.emit(Instr::Not { dst: not_yet, src: matched });
            let go = self.new_reg(bool_ty);
            // `not_yet && hit` without short-circuiting: both are plain values.
            let (t, join) = (self.new_block(), self.new_block());
            self.emit(Instr::Mov { dst: go, src: not_yet });
            self.terminate(Term::Branch { cond: not_yet, then_: t, else_: join });
            self.switch_to(t);
            self.emit(Instr::Mov { dst: go, src: hit });
            self.terminate(Term::Jump(join));
            self.switch_to(join);
            let arm_children = refs(arm_children);
            self.ui_if(go, |this| {
                let yes = this.const_reg(bool_ty, Const::Bool(true));
                this.emit(Instr::Mov { dst: matched, src: yes });
                this.ui_children(cx, &arm_children, depth)
            })?;
        }
        Ok(())
    }

    /// Emits `UiOpen` for an element whose modifiers are `expanded`. `pre` are
    /// expressions already evaluated.
    fn ui_open(
        &mut self,
        tag: &str,
        attributes: &[UiAttribute],
        expanded: &[&'a UiModifier],
        pre: Vec<(&'a Expr, Reg)>,
    ) -> LResult<()> {
        let mut slots = Vec::new();
        let mut values = Vec::new();
        // `key::{r}` / `key::{r.id}` parse as a one-name tuple; make them expressions.
        let owned: Vec<UiModifier> = expanded.iter().map(|m| key_as_expr(m)).collect();
        let mut exprs: Vec<&Expr> = Vec::new();
        for (orig, new) in expanded.iter().zip(&owned) {
            // Unchanged modifiers keep their own expressions (the checker typed those).
            if matches!(orig.value, UiModifierValue::Tuple(_)) && matches!(new.value, UiModifierValue::Expr(_)) {
                exprs.extend(modifier_exprs_of(std::iter::once(new)));
            } else {
                exprs.extend(modifier_exprs_of(std::iter::once(*orig)));
            }
        }
        for e in exprs {
            let reg = match pre.iter().find(|(p, _)| std::ptr::eq(*p, e)) {
                Some((_, r)) => *r,
                None => self.expr(e, None)?,
            };
            let slot = match self.tk(self.reg_ty(reg)) {
                TyKind::Num(_) => UiSlot::Number,
                TyKind::Str => UiSlot::Str,
                TyKind::Bool => UiSlot::Bool,
                _ => UiSlot::Unused,
            };
            if slot != UiSlot::Unused {
                values.push(reg);
            }
            slots.push(slot);
        }
        // A variable as an attribute value (`placeholder||hint`) is an expression like any other.
        let attributes: Vec<UiAttribute> = attributes
            .iter()
            .map(|a| {
                let mut a = a.clone();
                if let UiAttrValue::Ident(name) = &a.value {
                    if crate::typed::ui::VALUE_ATTRS.contains(&a.name.as_str()) && name != "true" && name != "false" {
                        a.value = UiAttrValue::Expr(Expr::Ident(name.clone(), tint_ast::Span::dummy()));
                    }
                }
                a
            })
            .collect();
        for e in crate::typed::ui::attr_exprs(&attributes) {
            // The checker does not visit attribute expressions: a struct literal names its own type.
            let hint = match e {
                Expr::StructInit { name, .. } => Some(self.ast_ty(&tint_ast::Type::Simple(name.clone()))?),
                _ => None,
            };
            let mut reg = self.expr(e, hint)?;
            let is_props = attributes.iter().any(|a| a.name == "props" && matches!(&a.value, UiAttrValue::Expr(x) if std::ptr::eq(x, e)));
            if is_props {
                let text = self.str_ty();
                reg = self.rt(RtFn::PropsJson, vec![reg], text);
            }
            let slot = match self.tk(self.reg_ty(reg)) {
                TyKind::Num(_) => UiSlot::Number,
                TyKind::Str => UiSlot::Str,
                TyKind::Bool => UiSlot::Bool,
                _ => UiSlot::Unused,
            };
            if slot != UiSlot::Unused {
                values.push(reg);
            }
            slots.push(slot);
        }
        let template = self.module.ui_templates.len() as u32;
        self.module.ui_templates.push(UiTemplate::Element(UiElementTemplate {
            tag: tag.to_string(),
            modifiers: owned.clone(),
            attributes,
            slots,
        }));
        self.emit(Instr::UiOpen { template, values });
        Ok(())
    }
}

fn key_as_expr(m: &UiModifier) -> UiModifier {
    if m.path.len() == 1 && m.path[0] == "key" {
        if let UiModifierValue::Tuple(items) = &m.value {
            if let [UiModifierValue::Ident(name)] = items.as_slice() {
                let mut parts = name.split('.');
                let span = m.span;
                let mut e = Expr::Ident(parts.next().unwrap_or("").to_string(), span);
                for f in parts {
                    e = Expr::Field { target: Box::new(e), field: f.to_string(), span };
                }
                return UiModifier { path: m.path.clone(), value: UiModifierValue::Expr(e), span };
            }
        }
    }
    m.clone()
}
