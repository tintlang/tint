//! Expressions.

use super::*;
use tint_ast::{Block, Expr, StringPart, StructInitField};

/// Can values of type `a` be turned into `b` by converting numbers only?
fn compat(types: &TypeTable, a: TyId, b: TyId) -> bool {
    if a == b {
        return true;
    }
    match (types.kind(a), types.kind(b)) {
        (TyKind::Num(_), TyKind::Num(_)) => true,
        (TyKind::List(x), TyKind::List(y)) | (TyKind::Map(x), TyKind::Map(y)) => compat(types, *x, *y),
        (TyKind::Tuple(xs), TyKind::Tuple(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| compat(types, *x, *y))
        }
        (TyKind::Adt(x), TyKind::Adt(y)) => {
            let (x, y) = (types.adt(*x), types.adt(*y));
            x.base == y.base
                && x.args.len() == y.args.len()
                && x.args.iter().zip(&y.args).all(|(p, q)| compat(types, *p, *q))
        }
        (TyKind::Fn(p1, r1), TyKind::Fn(p2, r2)) => {
            p1.len() == p2.len()
                && p1.iter().zip(p2).all(|(p, q)| compat(types, *p, *q))
                && compat(types, *r1, *r2)
        }
        _ => false,
    }
}

impl<'a> Lowerer<'a> {
    pub fn compat(&self, a: TyId, b: TyId) -> bool {
        compat(&self.module.types, a, b)
    }

    /// Type the checker gave `e`.
    pub fn nat(&mut self, e: &Expr) -> LResult<TyId> {
        let ty = match self.model.type_of(e) {
            Some(ty) => ty.clone(),
            None => return self.err(Some(e.span()), "expression has no recorded type"),
        };
        self.conv(&ty).map_err(|mut error| {
            error.span = error.span.or(Some(e.span()));
            error
        })
    }

    /// The type an expression is built with: the context's, when it only
    /// differs in number kinds from the checker's.
    pub fn pick(&mut self, e: &Expr, hint: Option<TyId>) -> LResult<TyId> {
        let nat = self.nat(e)?;
        Ok(match hint {
            Some(h) if self.compat(nat, h) => h,
            _ => nat,
        })
    }

    /// Lowers `e` and converts the result to `target`.
    pub fn expr_as(&mut self, e: &Expr, target: TyId) -> LResult<Reg> {
        let reg = self.expr(e, Some(target))?;
        self.coerce(reg, target, Some(e.span()))
    }

    /// Converts a value to `target`: identical types pass through, numbers are
    /// converted with a checked cast.
    pub fn coerce(&mut self, reg: Reg, target: TyId, span: Option<Span>) -> LResult<Reg> {
        let have = self.reg_ty(reg);
        if have == target {
            return Ok(reg);
        }
        if let (TyKind::Num(_), TyKind::Num(_)) = (self.tk(have), self.tk(target)) {
            let dst = self.new_reg(target);
            self.emit(Instr::Cast { dst, src: reg });
            return Ok(dst);
        }
        let (want, found) = (self.show(target), self.show(have));
        self.err(span, format!("type mismatch: expected {want}, found {found}"))
    }

    /// A value that is only read right away needs no private copy.
    pub fn expr_ref(&mut self, e: &Expr) -> LResult<Reg> {
        match e {
            Expr::Paren(inner, _) => self.expr_ref(inner),
            Expr::Ident(name, _) => match self.resolve_local(name) {
                Some(reg) => Ok(reg),
                None => self.expr(e, None),
            },
            Expr::SelfKw(_) => match self.resolve_local("self") {
                Some(reg) => Ok(reg),
                None => self.expr(e, None),
            },
            _ => self.expr(e, None),
        }
    }

    pub fn block(&mut self, block: &Block, hint: Option<TyId>) -> LResult<Reg> {
        self.push_scope();
        let result = self.block_in_scope(block, hint);
        self.pop_scope();
        result
    }

    pub fn block_in_scope(&mut self, block: &Block, hint: Option<TyId>) -> LResult<Reg> {
        let count = block.stmts.len();
        let mut value = None;
        for (index, stmt) in block.stmts.iter().enumerate() {
            match stmt {
                Stmt::Expr(e) if index + 1 == count => value = Some(self.expr(e, hint)?),
                other => self.stmt(other)?,
            }
        }
        if self.diverged() {
            let ty = match hint {
                Some(h) => h,
                None => self.unit_ty(),
            };
            return Ok(self.undef(ty));
        }
        match value {
            Some(v) => Ok(v),
            None => Ok(self.unit_reg()),
        }
    }

    /// Has control already left the current block for good?
    pub fn diverged(&mut self) -> bool {
        let fb = self.f();
        fb.blocks[fb.cur].1.is_some()
    }

    pub fn expr(&mut self, e: &Expr, hint: Option<TyId>) -> LResult<Reg> {
        match e {
            Expr::Number(text, span) => {
                let ty = self.pick(e, hint)?;
                self.number_literal(text, ty, false, *span)
            }
            Expr::String(text, _) => {
                let ty = self.str_ty();
                Ok(self.const_reg(ty, Const::Str(text.clone())))
            }
            Expr::Bool(b, _) => {
                let ty = self.bool_ty();
                Ok(self.const_reg(ty, Const::Bool(*b)))
            }
            Expr::Unit(_) => Ok(self.unit_reg()),
            Expr::Ident(name, span) => self.ident(name, *span),
            Expr::SelfKw(span) => self.ident("self", *span),
            Expr::Paren(inner, _) => self.expr(inner, hint),
            Expr::InterpolatedString { parts, .. } => {
                let mut regs = Vec::new();
                for part in parts {
                    match part {
                        StringPart::Text(text) => {
                            let ty = self.str_ty();
                            regs.push(self.const_reg(ty, Const::Str(text.clone())));
                        }
                        StringPart::Expr(inner) => {
                            let v = self.expr(inner, None)?;
                            regs.push(self.to_str(v));
                        }
                    }
                }
                let ty = self.str_ty();
                Ok(self.rt(RtFn::StrConcat, regs, ty))
            }
            Expr::Unary { op, expr: inner, span } => self.unary(op, inner, e, hint, *span),
            Expr::Binary { left, op, right, span } => self.binary(op, left, right, e, hint, *span),
            Expr::Cast { expr: inner, ty, span } => {
                let target = self.ast_ty(ty)?;
                if !matches!(self.tk(target), TyKind::Num(_)) {
                    return self.err(Some(*span), "`as` needs a numeric type");
                }
                let v = self.expr(inner, None)?;
                if !matches!(self.tk(self.reg_ty(v)), TyKind::Num(_)) {
                    return self.err(Some(*span), "`as` needs a numeric value");
                }
                let dst = self.new_reg(target);
                self.emit(Instr::Cast { dst, src: v });
                Ok(dst)
            }
            Expr::If { cond, then, else_, .. } => {
                let ty = self.pick(e, hint)?;
                let bool_ty = self.bool_ty();
                let c = self.expr_as(cond, bool_ty)?;
                let result = self.new_reg(ty);
                let (then_b, else_b, join) = (self.new_block(), self.new_block(), self.new_block());
                self.terminate(Term::Branch { cond: c, then_: then_b, else_: else_b });
                for (target, block) in [(then_b, then), (else_b, else_)] {
                    self.switch_to(target);
                    let v = self.block(block, Some(ty))?;
                    let v = self.coerce(v, ty, Some(block.span))?;
                    self.emit(Instr::Mov { dst: result, src: v });
                    self.terminate(Term::Jump(join));
                }
                self.switch_to(join);
                Ok(result)
            }
            Expr::Match { scrutinee, arms, .. } => {
                let ty = self.pick(e, hint)?;
                self.match_expr(scrutinee, arms, ty, e.span())
            }
            Expr::Block(block, _) => self.block(block, hint),
            Expr::Lambda { params, body, span } => self.lambda(e, params, body, hint, *span),
            Expr::Array { items, .. } => {
                let ty = self.pick(e, hint)?;
                let TyKind::List(item) = self.tk(ty) else {
                    return self.err(Some(e.span()), "array literal without a list type");
                };
                let mut regs = Vec::new();
                for i in items {
                    regs.push(self.expr_as(i, item)?);
                }
                let dst = self.new_reg(ty);
                self.emit(Instr::List { dst, items: regs });
                Ok(dst)
            }
            Expr::Tuple { items, .. } => {
                let ty = self.pick(e, hint)?;
                let TyKind::Tuple(tys) = self.tk(ty) else {
                    return self.err(Some(e.span()), "tuple literal without a tuple type");
                };
                let mut regs = Vec::new();
                for (i, t) in items.iter().zip(tys) {
                    regs.push(self.expr_as(i, t)?);
                }
                let dst = self.new_reg(ty);
                self.emit(Instr::Tuple { dst, items: regs });
                Ok(dst)
            }
            Expr::MapInit { entries, .. } => {
                let ty = self.pick(e, hint)?;
                let TyKind::Map(item) = self.tk(ty) else {
                    return self.err(Some(e.span()), "map literal without a map type");
                };
                let mut regs = Vec::new();
                for (key, value) in entries {
                    regs.push((key.clone(), self.expr_as(value, item)?));
                }
                let dst = self.new_reg(ty);
                self.emit(Instr::Map { dst, entries: regs });
                Ok(dst)
            }
            Expr::Field { target, field, span } => {
                let base = self.expr_ref(target)?;
                let base_ty = self.reg_ty(base);
                let TyKind::Adt(adt) = self.tk(base_ty) else {
                    return self.err(Some(*span), format!("`.{field}` on {}", self.show(base_ty)));
                };
                let def = self.module.types.adt(adt);
                let Some(index) = def.field_index(field) else {
                    return self.err(Some(*span), format!("`{}` has no field `{field}`", self.show(base_ty)));
                };
                let ty = def.struct_fields()[index as usize].ty;
                let dst = self.new_reg(ty);
                self.emit(Instr::Get { dst, base, proj: Proj::Field(index) });
                Ok(dst)
            }
            Expr::TupleIndex { target, index, span } => {
                let base = self.expr_ref(target)?;
                let base_ty = self.reg_ty(base);
                let TyKind::Tuple(tys) = self.tk(base_ty) else {
                    return self.err(Some(*span), "tuple index on a non-tuple");
                };
                let Some(ty) = tys.get(*index).copied() else {
                    return self.err(Some(*span), "tuple index out of range");
                };
                let dst = self.new_reg(ty);
                self.emit(Instr::Get { dst, base, proj: Proj::Tuple(*index as u32) });
                Ok(dst)
            }
            Expr::Index { target, index, span } => {
                let base = self.expr_ref(target)?;
                let base_ty = self.reg_ty(base);
                match self.tk(base_ty) {
                    TyKind::List(item) => {
                        let num = self.num_ty(NumKind::Num);
                        let i = self.expr(index, Some(num))?;
                        if !matches!(self.tk(self.reg_ty(i)), TyKind::Num(_)) {
                            return self.err(Some(*span), "list index must be a number");
                        }
                        let dst = self.new_reg(item);
                        self.emit(Instr::Get { dst, base, proj: Proj::Index(i) });
                        Ok(dst)
                    }
                    TyKind::Map(item) => {
                        let key = self.key_reg(index)?;
                        let dst = self.new_reg(item);
                        self.emit(Instr::Get { dst, base, proj: Proj::Key(key) });
                        Ok(dst)
                    }
                    _ => self.err(Some(*span), format!("cannot index {}", self.show(base_ty))),
                }
            }
            Expr::Call { target, args, span } => self.call(e, target, args, hint, *span),
            Expr::Namespace { base, item, span } => self.namespace(e, base, item, hint, *span),
            Expr::StructInit { name, fields, span } => self.struct_init(e, name, fields, hint, *span),
            Expr::StructUpdate { base, updates, span } => {
                let ty = self.pick(e, hint)?;
                let v = self.expr(base, Some(ty))?;
                let v = self.coerce(v, ty, Some(*span))?;
                let result = self.mov(v);
                let TyKind::Adt(adt) = self.tk(ty) else {
                    return self.err(Some(*span), "struct update on a non-struct");
                };
                for field in updates {
                    let (name, value) = init_parts(field);
                    let def = self.module.types.adt(adt);
                    let Some(index) = def.field_index(name) else {
                        return self.err(Some(*span), format!("no field `{name}`"));
                    };
                    let fty = def.struct_fields()[index as usize].ty;
                    let src = self.expr_as(value, fty)?;
                    self.emit(Instr::Set { base: result, proj: Proj::Field(index), src });
                }
                Ok(result)
            }
            Expr::VariantInit { enum_name, variant, fields, span } => {
                self.variant_init(e, enum_name, variant, fields, hint, *span)
            }
            Expr::Try { expr: inner, span } => self.try_expr(inner, *span),
            Expr::Borrow { target, .. } => self.expr(target, hint),
            Expr::NamedArg { span, .. } => self.err(Some(*span), "a named argument outside a call of a known function"),
        }
    }

    // ----------------------------------------------------------- pieces

    pub fn number_literal(&mut self, text: &str, ty: TyId, negate: bool, span: Span) -> LResult<Reg> {
        let TyKind::Num(kind) = self.tk(ty) else {
            return self.err(Some(span), format!("number literal {text} used as {}", self.show(ty)));
        };
        let bad = |this: &Self| this.error(Some(span), format!("{text} is not a valid {}", kind.name()));
        let value = if kind.is_int() {
            let wide: i128 = match text.parse::<i128>() {
                Ok(n) => n,
                Err(_) => {
                    let f: f64 = text.parse().map_err(|_| bad(self))?;
                    if !f.is_finite() || f.fract() != 0.0 {
                        return Err(bad(self));
                    }
                    f as i128
                }
            };
            let wide = if negate { -wide } else { wide };
            let (lo, hi) = kind.int_range();
            if wide < lo || wide > hi {
                return Err(bad(self));
            }
            Const::Int(if kind == NumKind::U64 { (wide as u64) as i64 } else { wide as i64 })
        } else {
            let f: f64 = text.parse().map_err(|_| bad(self))?;
            let f = if negate { -f } else { f };
            Const::Float(if kind == NumKind::F32 { (f as f32) as f64 } else { f })
        };
        Ok(self.const_reg(ty, value))
    }

    pub(super) fn ident(&mut self, name: &str, span: Span) -> LResult<Reg> {
        if let Some(reg) = self.resolve_local(name) {
            return Ok(self.mov(reg));
        }
        if let Some(global) = self.globals.get(name).copied() {
            let ty = self.module.globals[global.0 as usize].ty;
            let dst = self.new_reg(ty);
            self.emit(Instr::GlobalGet { dst, global });
            return Ok(dst);
        }
        if let Some(info) = self.fns.get(name).cloned() {
            let ty = self.module.types.func(info.params.clone(), info.ret);
            let dst = self.new_reg(ty);
            self.emit(Instr::Closure { dst, func: info.id, captures: Vec::new() });
            return Ok(dst);
        }
        self.err(Some(span), format!("`{name}` is not bound to anything that can be lowered"))
    }

    pub fn to_str(&mut self, v: Reg) -> Reg {
        if matches!(self.tk(self.reg_ty(v)), TyKind::Str) {
            return v;
        }
        let ty = self.str_ty();
        let dst = self.new_reg(ty);
        self.emit(Instr::ToStr { dst, src: v });
        dst
    }

    /// A map key: the text of whatever was given.
    pub fn key_reg(&mut self, e: &Expr) -> LResult<Reg> {
        let v = self.expr(e, None)?;
        Ok(self.to_str(v))
    }

    fn unary(&mut self, op: &str, inner: &Expr, e: &Expr, hint: Option<TyId>, span: Span) -> LResult<Reg> {
        match op {
            "!" => {
                let bool_ty = self.bool_ty();
                let v = self.expr_as(inner, bool_ty)?;
                let dst = self.new_reg(bool_ty);
                self.emit(Instr::Not { dst, src: v });
                Ok(dst)
            }
            "-" => {
                let ty = self.pick(e, hint)?;
                let mut probe = inner;
                while let Expr::Paren(p, _) = probe {
                    probe = p;
                }
                if let Expr::Number(text, lit_span) = probe {
                    return self.number_literal(text, ty, true, *lit_span);
                }
                let v = self.expr(inner, Some(ty))?;
                let v = self.coerce(v, ty, Some(span))?;
                if !matches!(self.tk(ty), TyKind::Num(_)) {
                    return self.err(Some(span), "negation of a non-number");
                }
                let dst = self.new_reg(ty);
                self.emit(Instr::Neg { dst, src: v });
                Ok(dst)
            }
            other => self.err(Some(span), format!("unary `{other}` cannot be lowered")),
        }
    }

    fn binary(&mut self, op: &str, left: &Expr, right: &Expr, e: &Expr, hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let bool_ty = self.bool_ty();
        match op {
            "&&" | "||" => {
                let a = self.expr_as(left, bool_ty)?;
                let result = self.new_reg(bool_ty);
                self.emit(Instr::Mov { dst: result, src: a });
                let (rhs, join) = (self.new_block(), self.new_block());
                let term = if op == "&&" {
                    Term::Branch { cond: a, then_: rhs, else_: join }
                } else {
                    Term::Branch { cond: a, then_: join, else_: rhs }
                };
                self.terminate(term);
                self.switch_to(rhs);
                let b = self.expr_as(right, bool_ty)?;
                self.emit(Instr::Mov { dst: result, src: b });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                let (tl, tr) = (self.nat(left)?, self.nat(right)?);
                let common = match (self.tk(tl), self.tk(tr)) {
                    (TyKind::Num(a), TyKind::Num(b)) => {
                        if a != NumKind::Num {
                            tl
                        } else if b != NumKind::Num {
                            tr
                        } else {
                            tl
                        }
                    }
                    _ => tl,
                };
                let a = self.expr(left, Some(common))?;
                let b = self.expr(right, Some(common))?;
                let (a, b) = if matches!(self.tk(common), TyKind::Num(_)) {
                    (self.coerce(a, common, Some(span))?, self.coerce(b, common, Some(span))?)
                } else {
                    (a, b)
                };
                if self.reg_ty(a) != self.reg_ty(b) {
                    let (x, y) = (self.show(self.reg_ty(a)), self.show(self.reg_ty(b)));
                    return self.err(Some(span), format!("cannot compare {x} with {y}"));
                }
                let op = match op {
                    "==" => CmpOp::Eq,
                    "!=" => CmpOp::Ne,
                    "<" => CmpOp::Lt,
                    ">" => CmpOp::Gt,
                    "<=" => CmpOp::Le,
                    _ => CmpOp::Ge,
                };
                let dst = self.new_reg(bool_ty);
                self.emit(Instr::Cmp { dst, op, a, b });
                Ok(dst)
            }
            "+" | "-" | "*" | "/" | "%" => {
                let (tl, tr) = (self.nat(left)?, self.nat(right)?);
                if op == "+" && (matches!(self.tk(tl), TyKind::Str) || matches!(self.tk(tr), TyKind::Str)) {
                    let a = self.expr(left, None)?;
                    let a = self.to_str(a);
                    let b = self.expr(right, None)?;
                    let b = self.to_str(b);
                    let ty = self.str_ty();
                    return Ok(self.rt(RtFn::StrConcat, vec![a, b], ty));
                }
                let ty = self.pick(e, hint)?;
                let TyKind::Num(kind) = self.tk(ty) else {
                    return self.err(Some(span), format!("`{op}` on {}", self.show(ty)));
                };
                let a = self.expr_as(left, ty)?;
                let b = self.expr_as(right, ty)?;
                Ok(self.arith(bin_op(op), kind, ty, a, b))
            }
            other => self.err(Some(span), format!("operator `{other}` cannot be lowered")),
        }
    }

    pub fn arith(&mut self, op: BinOp, kind: NumKind, ty: TyId, a: Reg, b: Reg) -> Reg {
        let dst = self.new_reg(ty);
        self.emit(Instr::Bin { dst, op, kind, a, b });
        dst
    }

    fn struct_init(&mut self, e: &Expr, name: &str, fields: &[StructInitField], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let ty = self.pick(e, hint)?;
        let TyKind::Adt(adt) = self.tk(ty) else {
            return self.err(Some(span), format!("`{name}` is not a struct"));
        };
        let defs: Vec<FieldDef> = self.module.types.adt(adt).struct_fields().to_vec();
        let mut slots: Vec<Option<Reg>> = vec![None; defs.len()];
        for field in fields {
            let (field_name, value) = init_parts(field);
            let Some(index) = defs.iter().position(|d| d.name == field_name) else {
                return self.err(Some(span), format!("`{name}` has no field `{field_name}`"));
            };
            slots[index] = Some(self.expr_as(value, defs[index].ty)?);
        }
        let mut regs = Vec::new();
        for (slot, def) in slots.into_iter().zip(&defs) {
            match slot {
                Some(r) => regs.push(r),
                None => return self.err(Some(span), format!("field `{}` of `{name}` is missing", def.name)),
            }
        }
        let dst = self.new_reg(ty);
        self.emit(Instr::Struct { dst, adt, fields: regs });
        Ok(dst)
    }

    fn variant_init(&mut self, e: &Expr, enum_name: &str, variant: &str, fields: &[StructInitField], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let ty = self.pick(e, hint)?;
        let TyKind::Adt(adt) = self.tk(ty) else {
            return self.err(Some(span), format!("`{enum_name}` is not an enum"));
        };
        let def = self.module.types.adt(adt);
        let Some(index) = def.variant_index(variant) else {
            return self.err(Some(span), format!("`{enum_name}` has no variant `{variant}`"));
        };
        let vdef = def.variants()[index as usize].clone();
        if vdef.fields.len() != fields.len() {
            return self.err(Some(span), format!("`{enum_name}::{variant}` takes {} fields", vdef.fields.len()));
        }
        let mut slots: Vec<Option<Reg>> = vec![None; vdef.fields.len()];
        for (position, field) in fields.iter().enumerate() {
            let (field_name, value) = init_parts(field);
            let slot = vdef.fields.iter().position(|f| f.name == field_name).unwrap_or(position);
            slots[slot] = Some(self.expr_as(value, vdef.fields[slot].ty)?);
        }
        let mut regs = Vec::new();
        for slot in slots {
            match slot {
                Some(r) => regs.push(r),
                None => return self.err(Some(span), "a field of the variant is initialized twice"),
            }
        }
        let dst = self.new_reg(ty);
        self.emit(Instr::Variant { dst, adt, variant: index, fields: regs });
        Ok(dst)
    }

    /// `Enum::Variant` without arguments.
    fn namespace(&mut self, e: &Expr, base: &Expr, item: &str, hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let Expr::Ident(enum_name, _) = base else {
            return self.err(Some(span), "only `Enum::Variant` paths can be lowered");
        };
        let ty = self.pick(e, hint)?;
        let TyKind::Adt(adt) = self.tk(ty) else {
            return self.err(Some(span), format!("`{enum_name}::{item}` is not a value"));
        };
        let def = self.module.types.adt(adt);
        let Some(index) = def.variant_index(item) else {
            return self.err(Some(span), format!("`{enum_name}` has no variant `{item}`"));
        };
        if !def.variants()[index as usize].fields.is_empty() {
            return self.err(Some(span), format!("`{enum_name}::{item}` needs its fields"));
        }
        let dst = self.new_reg(ty);
        self.emit(Instr::Variant { dst, adt, variant: index, fields: Vec::new() });
        Ok(dst)
    }

    fn try_expr(&mut self, inner: &Expr, span: Span) -> LResult<Reg> {
        let v = self.expr(inner, None)?;
        let v_ty = self.reg_ty(v);
        let TyKind::Adt(adt) = self.tk(v_ty) else {
            return self.err(Some(span), "`?` needs an Option or a Result");
        };
        let base = self.module.types.adt(adt).base.clone();
        let (ok_index, failure_variant) = match base.as_str() {
            "Option" => (1u32, "None"),
            "Result" => (0u32, "Err"),
            _ => return self.err(Some(span), "`?` needs an Option or a Result"),
        };
        let ret = self.f().ret;
        let TyKind::Adt(ret_adt) = self.tk(ret) else {
            return self.err(Some(span), format!("`?` in a function returning {}", self.show(ret)));
        };
        if self.module.types.adt(ret_adt).base != base {
            return self.err(
                Some(span),
                format!("`?` on {} in a function returning {}", self.show(v_ty), self.show(ret)),
            );
        }
        let payload_ty = self.module.types.adt(adt).variants()[ok_index as usize].fields[0].ty;
        let tag_ty = self.num_ty(NumKind::I64);
        let tag = self.new_reg(tag_ty);
        self.emit(Instr::Tag { dst: tag, src: v });
        let (ok_b, err_b) = (self.new_block(), self.new_block());
        self.terminate(Term::Switch { value: tag, cases: vec![(ok_index as i64, ok_b)], default: err_b });

        self.switch_to(err_b);
        let failure_index = self.module.types.adt(ret_adt).variant_index(failure_variant).unwrap();
        let mut fields = Vec::new();
        if base == "Result" {
            let err_ty = self.module.types.adt(adt).variants()[1].fields[0].ty;
            let want = self.module.types.adt(ret_adt).variants()[1].fields[0].ty;
            let e = self.new_reg(err_ty);
            self.emit(Instr::Payload { dst: e, src: v, variant: 1, index: 0 });
            fields.push(self.coerce(e, want, Some(span))?);
        }
        let failure = self.new_reg(ret);
        self.emit(Instr::Variant { dst: failure, adt: ret_adt, variant: failure_index, fields });
        self.finish_return(failure, span)?;

        self.switch_to(ok_b);
        let dst = self.new_reg(payload_ty);
        self.emit(Instr::Payload { dst, src: v, variant: ok_index, index: 0 });
        Ok(dst)
    }

    /// `return value`: converts to the declared type and, in methods that
    /// hand back their receiver, pairs it with the result.
    pub fn finish_return(&mut self, value: Reg, span: Span) -> LResult<()> {
        let ret = self.f().ret;
        let v = self.coerce(value, ret, Some(span))?;
        if self.f().inout {
            let me = self.f().self_reg.expect("inout method without self");
            let me_ty = self.reg_ty(me);
            let tuple_ty = self.module.types.tuple(vec![ret, me_ty]);
            let pair = self.new_reg(tuple_ty);
            self.emit(Instr::Tuple { dst: pair, items: vec![v, me] });
            self.terminate(Term::Return(pair));
        } else {
            self.terminate(Term::Return(v));
        }
        Ok(())
    }

    fn lambda(&mut self, e: &Expr, params: &[String], body: &Expr, hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let ty = self.pick(e, hint)?;
        let TyKind::Fn(param_tys, ret) = self.tk(ty) else {
            return self.err(Some(span), "lambda without a function type");
        };
        if param_tys.len() != params.len() {
            return self.err(Some(span), "lambda parameter count differs from its type");
        }
        self.lambdas += 1;
        let name = format!("{}$lambda{}", self.stack.last().unwrap().name, self.lambdas);
        let id = self.reserve_func(&name, FuncKind::Lambda, ret);
        let mut fb = FnB::new(name, FuncKind::Lambda, ret);
        fb.is_lambda = true;
        self.stack.push(fb);
        for (param, pty) in params.iter().zip(&param_tys) {
            let reg = self.new_reg(*pty);
            self.f().params.push(reg);
            self.f().define(param, reg);
        }
        let outcome = (|| -> LResult<()> {
            let v = self.expr(body, Some(ret))?;
            self.finish_return(v, span)
        })();
        let fb = self.stack.pop().unwrap();
        outcome?;
        let captures: Vec<Reg> = fb.captures.iter().map(|c| c.outer).collect();
        self.module.funcs[id.0 as usize] = fb.finish(ret);
        let dst = self.new_reg(ty);
        self.emit(Instr::Closure { dst, func: id, captures });
        Ok(dst)
    }
}

pub(crate) fn init_parts(field: &StructInitField) -> (&str, &Expr) {
    match field {
        StructInitField::Assign { name, expr, .. } | StructInitField::Tint { name, expr, .. } => (name, expr),
    }
}

pub(crate) fn bin_op(op: &str) -> BinOp {
    match op {
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        _ => BinOp::Rem,
    }
}

