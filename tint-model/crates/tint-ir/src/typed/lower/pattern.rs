//! Patterns and `match`.

use super::*;
use tint_ast::{Expr, MatchArm, PatternField};

impl<'a> Lowerer<'a> {
    /// Continues in the current block when `val` (of type `ty`) matches and
    /// jumps to `fail` when it does not; binds the pattern's names in the
    /// current scope.
    pub fn bind_pattern(&mut self, pat: &Pattern, val: Reg, ty: TyId, fail: BlockId) -> LResult<()> {
        match pat {
            Pattern::Wildcard(_) => Ok(()),
            Pattern::Mut { inner, .. } => self.bind_pattern(inner, val, ty, fail),
            Pattern::Typed { pat, ty: declared, span } => {
                let declared = self.ast_ty(declared)?;
                let val = self.coerce(val, declared, Some(*span))?;
                self.bind_pattern(pat, val, declared, fail)
            }
            Pattern::Ident(name, _) => {
                if let TyKind::Adt(adt) = self.tk(ty) {
                    if let Some(index) = self.module.types.adt(adt).variant_index(name) {
                        return self.test_variant(val, index, fail);
                    }
                }
                let reg = self.new_reg(ty);
                self.emit(Instr::Mov { dst: reg, src: val });
                self.f().define(name, reg);
                Ok(())
            }
            Pattern::Number(text, span) => {
                let lit = self.number_literal(text, ty, false, *span)?;
                self.test_equal(val, lit, fail);
                Ok(())
            }
            Pattern::String(text, _) => {
                let str_ty = self.str_ty();
                if ty != str_ty {
                    return self.err(Some(pat.span()), "string pattern on a non-string");
                }
                let lit = self.const_reg(str_ty, Const::Str(text.clone()));
                self.test_equal(val, lit, fail);
                Ok(())
            }
            Pattern::Tuple(items, span) => {
                let TyKind::Tuple(tys) = self.tk(ty) else {
                    return self.err(Some(*span), "tuple pattern on a non-tuple");
                };
                if tys.len() != items.len() {
                    return self.err(Some(*span), "tuple pattern of the wrong length");
                }
                for (index, (item, item_ty)) in items.iter().zip(tys).enumerate() {
                    let part = self.new_reg(item_ty);
                    self.emit(Instr::Get { dst: part, base: val, proj: Proj::Tuple(index as u32) });
                    self.bind_pattern(item, part, item_ty, fail)?;
                }
                Ok(())
            }
            Pattern::Variant { name, args, span } => {
                let (adt, index) = self.enum_variant(ty, name, *span)?;
                let fields = self.module.types.adt(adt).variants()[index as usize].fields.clone();
                if fields.len() != args.len() {
                    return self.err(Some(*span), format!("`{name}` has {} fields, the pattern {}", fields.len(), args.len()));
                }
                self.test_variant(val, index, fail)?;
                for (i, (arg, field)) in args.iter().zip(&fields).enumerate() {
                    let part = self.new_reg(field.ty);
                    self.emit(Instr::Payload { dst: part, src: val, variant: index, index: i as u32 });
                    self.bind_pattern(arg, part, field.ty, fail)?;
                }
                Ok(())
            }
            Pattern::Struct { name, fields, span } => {
                let TyKind::Adt(adt) = self.tk(ty) else {
                    return self.err(Some(*span), format!("`{name} {{ .. }}` pattern on {}", self.show(ty)));
                };
                let is_enum = self.module.types.adt(adt).is_enum();
                let (variant, defs) = if is_enum {
                    let (adt, index) = self.enum_variant(ty, name, *span)?;
                    self.test_variant(val, index, fail)?;
                    (Some(index), self.module.types.adt(adt).variants()[index as usize].fields.clone())
                } else {
                    (None, self.module.types.adt(adt).struct_fields().to_vec())
                };
                for field in fields {
                    let (field_name, sub) = match field {
                        PatternField::Shorthand { field, .. } => (field, None),
                        PatternField::Assign { field, pat, .. } => (field, Some(pat)),
                        PatternField::Rest(_) => continue,
                    };
                    let Some(index) = defs.iter().position(|d| &d.name == field_name) else {
                        return self.err(Some(field.span()), format!("`{name}` has no field `{field_name}`"));
                    };
                    let field_ty = defs[index].ty;
                    let part = self.new_reg(field_ty);
                    match variant {
                        Some(v) => self.emit(Instr::Payload { dst: part, src: val, variant: v, index: index as u32 }),
                        None => self.emit(Instr::Get { dst: part, base: val, proj: Proj::Field(index as u32) }),
                    }
                    match sub {
                        None => {
                            let reg = self.new_reg(field_ty);
                            self.emit(Instr::Mov { dst: reg, src: part });
                            self.f().define(field_name, reg);
                        }
                        Some(sub) => self.bind_pattern(sub, part, field_ty, fail)?,
                    }
                }
                Ok(())
            }
            Pattern::Map { span, .. } | Pattern::Group { span, .. } => {
                self.err(Some(*span), "map patterns cannot be lowered yet")
            }
        }
    }

    fn enum_variant(&mut self, ty: TyId, name: &str, span: Span) -> LResult<(AdtId, u32)> {
        let TyKind::Adt(adt) = self.tk(ty) else {
            return self.err(Some(span), format!("pattern `{name}` on {}", self.show(ty)));
        };
        match self.module.types.adt(adt).variant_index(name) {
            Some(index) => Ok((adt, index)),
            None => self.err(Some(span), format!("{} has no variant `{name}`", self.show(ty))),
        }
    }

    fn test_variant(&mut self, val: Reg, index: u32, fail: BlockId) -> LResult<()> {
        let tag_ty = self.num_ty(NumKind::I64);
        let tag = self.new_reg(tag_ty);
        self.emit(Instr::Tag { dst: tag, src: val });
        let cont = self.new_block();
        self.terminate(Term::Switch { value: tag, cases: vec![(index as i64, cont)], default: fail });
        self.switch_to(cont);
        Ok(())
    }

    fn test_equal(&mut self, a: Reg, b: Reg, fail: BlockId) {
        let bool_ty = self.bool_ty();
        let eq = self.new_reg(bool_ty);
        self.emit(Instr::Cmp { dst: eq, op: CmpOp::Eq, a, b });
        let cont = self.new_block();
        self.terminate(Term::Branch { cond: eq, then_: cont, else_: fail });
        self.switch_to(cont);
    }

    pub fn match_expr(&mut self, scrutinee: &Expr, arms: &[MatchArm], ty: TyId, _span: Span) -> LResult<Reg> {
        let s = self.expr(scrutinee, None)?;
        let s_ty = self.reg_ty(s);
        let result = self.new_reg(ty);
        let join = self.new_block();
        let bool_ty = self.bool_ty();
        for arm in arms {
            let next = self.new_block();
            self.push_scope();
            let outcome = (|| -> LResult<()> {
                self.bind_pattern(&arm.pattern, s, s_ty, next)?;
                if let Some(guard) = &arm.guard {
                    let g = self.expr_as(guard, bool_ty)?;
                    let ok = self.new_block();
                    self.terminate(Term::Branch { cond: g, then_: ok, else_: next });
                    self.switch_to(ok);
                }
                let v = self.expr(&arm.expr, Some(ty))?;
                let v = self.coerce(v, ty, Some(arm.expr.span()))?;
                self.emit(Instr::Mov { dst: result, src: v });
                self.terminate(Term::Jump(join));
                Ok(())
            })();
            self.pop_scope();
            outcome?;
            self.switch_to(next);
        }
        if matches!(self.tk(ty), TyKind::Unit) {
            let unit = self.unit_reg();
            self.emit(Instr::Mov { dst: result, src: unit });
            self.terminate(Term::Jump(join));
        } else {
            self.terminate(Term::Trap("no match arm matched".into()));
        }
        self.switch_to(join);
        Ok(result)
    }
}
