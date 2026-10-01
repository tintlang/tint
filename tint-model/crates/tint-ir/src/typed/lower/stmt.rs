//! Statements, loops and places (assignable locations).

use super::*;
use tint_ast::{Block, Expr, LetInit};

pub(crate) enum Root {
    Local(Reg),
    Global(GlobalId),
}

/// An assignable location: a variable and a path of projections into it. The
/// index and key registers are already evaluated.
pub(crate) struct Place {
    pub root: Root,
    pub path: Vec<(Proj, TyId)>,
    pub ty: TyId,
}

/// The chain of `Take`s made to reach a place, so `leave_place` can put
/// everything back.
pub(crate) struct Entered {
    chain: Vec<(Reg, Proj, Reg)>,
    global: Option<(GlobalId, Reg)>,
}

impl<'a> Lowerer<'a> {
    pub fn stmt(&mut self, stmt: &Stmt) -> LResult<()> {
        match stmt {
            Stmt::Let {
                pattern,
                ty,
                init,
                span,
            } => {
                // `let x: i64 = 5` keeps its annotation on the pattern; use it as
                // the initializer's type so a literal is built as an i64, not as
                // a `number` followed by a checked cast.
                let mut annotated = pattern;
                while let Pattern::Mut { inner, .. } = annotated {
                    annotated = inner;
                }
                let declared = match (ty, annotated) {
                    (Some(t), _) | (None, Pattern::Typed { ty: t, .. }) => Some(self.ast_ty(t)?),
                    _ => None,
                };
                let init_expr = match init {
                    LetInit::Assign(e) | LetInit::Tint(e) => e,
                };
                let value = match declared {
                    Some(t) => self.expr_as(init_expr, t)?,
                    None => self.expr(init_expr, None)?,
                };
                let value_ty = self.reg_ty(value);
                let fail = self.trap_block("let pattern did not match");
                self.bind_pattern(pattern, value, value_ty, fail)?;
                let _ = span;
                Ok(())
            }
            Stmt::Assign { lhs, rhs, span } => {
                let hint = self.nat(lhs).ok();
                let value = self.expr(rhs, hint)?;
                let Some(place) = self.place_of(lhs)? else {
                    return self.err(Some(*span), "invalid assignment target");
                };
                let value = self.coerce(value, place.ty, Some(*span))?;
                self.store_place(&place, value);
                Ok(())
            }
            Stmt::CompoundAssign {
                name,
                op,
                expr,
                span,
            } => {
                let Some(place) = self.place_of(&Expr::Ident(name.clone(), *span))? else {
                    return self.err(Some(*span), format!("unknown variable `{name}`"));
                };
                let current = self.read_place(&place);
                let ty = place.ty;
                let rhs = self.expr(expr, Some(ty))?;
                let result = match self.tk(ty) {
                    TyKind::Num(kind) => {
                        let rhs = self.coerce(rhs, ty, Some(*span))?;
                        let op = match op.as_str() {
                            "+=" => BinOp::Add,
                            "-=" => BinOp::Sub,
                            "*=" => BinOp::Mul,
                            "/=" => BinOp::Div,
                            "%=" => BinOp::Rem,
                            other => {
                                return self
                                    .err(Some(*span), format!("`{other}` cannot be lowered"))
                            }
                        };
                        self.arith(op, kind, ty, current, rhs)
                    }
                    TyKind::Str if op == "+=" => {
                        let rhs = self.to_str(rhs);
                        self.rt(RtFn::StrConcat, vec![current, rhs], ty)
                    }
                    _ => return self.err(Some(*span), format!("`{op}` on {}", self.show(ty))),
                };
                self.store_place(&place, result);
                Ok(())
            }
            Stmt::Expr(e) => {
                self.discard = true;
                let r = self.expr(e, None);
                self.discard = false;
                r?;
                Ok(())
            }
            Stmt::If {
                cond, then, else_, ..
            } => {
                let bool_ty = self.bool_ty();
                let c = self.expr_as(cond, bool_ty)?;
                let (then_b, else_b, join) = (self.new_block(), self.new_block(), self.new_block());
                self.terminate(Term::Branch {
                    cond: c,
                    then_: then_b,
                    else_: else_b,
                });
                self.switch_to(then_b);
                self.block(then, None)?;
                self.terminate(Term::Jump(join));
                self.switch_to(else_b);
                if let Some(else_block) = else_ {
                    self.block(else_block, None)?;
                }
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(())
            }
            Stmt::While { cond, body, .. } => {
                let bool_ty = self.bool_ty();
                let (head, body_b, exit) = (self.new_block(), self.new_block(), self.new_block());
                self.terminate(Term::Jump(head));
                self.switch_to(head);
                let c = self.expr_as(cond, bool_ty)?;
                self.terminate(Term::Branch {
                    cond: c,
                    then_: body_b,
                    else_: exit,
                });
                self.switch_to(body_b);
                self.loop_body(body, exit, head)?;
                self.terminate(Term::Jump(head));
                self.switch_to(exit);
                Ok(())
            }
            Stmt::Loop { body, .. } => {
                let (head, exit) = (self.new_block(), self.new_block());
                self.terminate(Term::Jump(head));
                self.switch_to(head);
                self.loop_body(body, exit, head)?;
                self.terminate(Term::Jump(head));
                self.switch_to(exit);
                if !breaks(body) {
                    // Nothing leaves this loop except `return`.
                    self.terminate(Term::Trap(
                        "unreachable: a loop without `break` ended".into(),
                    ));
                }
                Ok(())
            }
            Stmt::For {
                var,
                start,
                end,
                body,
                span,
            } => {
                let i32_ty = self.num_ty(NumKind::I32);
                let bool_ty = self.bool_ty();
                let s = self.expr(start, None)?;
                let s = self.coerce(s, i32_ty, Some(*span))?;
                let e = self.expr(end, None)?;
                let e = self.coerce(e, i32_ty, Some(*span))?;
                let counter = self.mov(s);
                let (head, body_b, step, exit) = (
                    self.new_block(),
                    self.new_block(),
                    self.new_block(),
                    self.new_block(),
                );
                self.terminate(Term::Jump(head));
                self.switch_to(head);
                let more = self.new_reg(bool_ty);
                self.emit(Instr::Cmp {
                    dst: more,
                    op: CmpOp::Lt,
                    a: counter,
                    b: e,
                });
                self.terminate(Term::Branch {
                    cond: more,
                    then_: body_b,
                    else_: exit,
                });
                self.switch_to(body_b);
                self.push_scope();
                let item = self.mov(counter);
                self.f().define(var, item);
                let outcome = self.loop_body(body, exit, step);
                self.pop_scope();
                outcome?;
                self.terminate(Term::Jump(step));
                self.switch_to(step);
                let one = self.const_reg(i32_ty, Const::Int(1));
                let next = self.arith(BinOp::Add, NumKind::I32, i32_ty, counter, one);
                self.emit(Instr::Mov {
                    dst: counter,
                    src: next,
                });
                self.terminate(Term::Jump(head));
                self.switch_to(exit);
                Ok(())
            }
            Stmt::ForIn {
                var,
                iter,
                body,
                span,
            } => {
                let list = self.expr(iter, None)?;
                let TyKind::List(elem) = self.tk(self.reg_ty(list)) else {
                    return self.err(Some(*span), "`for` over a non-list");
                };
                let (num, bool_ty) = (self.num_ty(NumKind::Num), self.bool_ty());
                let n = self.rt(RtFn::ListLen, vec![list], num);
                let counter = self.const_reg(num, Const::Float(0.0));
                let (head, body_b, step, exit) = (
                    self.new_block(),
                    self.new_block(),
                    self.new_block(),
                    self.new_block(),
                );
                self.terminate(Term::Jump(head));
                self.switch_to(head);
                let more = self.new_reg(bool_ty);
                self.emit(Instr::Cmp {
                    dst: more,
                    op: CmpOp::Lt,
                    a: counter,
                    b: n,
                });
                self.terminate(Term::Branch {
                    cond: more,
                    then_: body_b,
                    else_: exit,
                });
                self.switch_to(body_b);
                self.push_scope();
                let item = self.new_reg(elem);
                self.emit(Instr::Get {
                    dst: item,
                    base: list,
                    proj: Proj::Index(counter),
                });
                self.f().define(var, item);
                let outcome = self.loop_body(body, exit, step);
                self.pop_scope();
                outcome?;
                self.terminate(Term::Jump(step));
                self.switch_to(step);
                let one = self.const_reg(num, Const::Float(1.0));
                let next = self.arith(BinOp::Add, NumKind::Num, num, counter, one);
                self.emit(Instr::Mov {
                    dst: counter,
                    src: next,
                });
                self.terminate(Term::Jump(head));
                self.switch_to(exit);
                Ok(())
            }
            Stmt::Break(span) => {
                let Some((exit, _)) = self.f().loops.last().copied() else {
                    return self.err(Some(*span), "`break` outside a loop");
                };
                self.terminate(Term::Jump(exit));
                Ok(())
            }
            Stmt::Continue(span) => {
                let Some((_, next)) = self.f().loops.last().copied() else {
                    return self.err(Some(*span), "`continue` outside a loop");
                };
                self.terminate(Term::Jump(next));
                Ok(())
            }
            Stmt::Return(e, span) => {
                let ret = self.f().ret;
                let v = self.expr(e, Some(ret))?;
                self.finish_return(v, *span)
            }
            Stmt::Match { span, .. } => self.err(
                Some(*span),
                "statement `match` is not produced by the parser",
            ),
        }
    }

    fn loop_body(&mut self, body: &Block, exit: BlockId, next: BlockId) -> LResult<()> {
        self.f().loops.push((exit, next));
        let outcome = self.block(body, None);
        self.f().loops.pop();
        outcome.map(|_| ())
    }

    // ------------------------------------------------------------ places

    pub fn place_of(&mut self, e: &Expr) -> LResult<Option<Place>> {
        match e {
            Expr::Paren(inner, _) => self.place_of(inner),
            Expr::Ident(name, span) => self.variable_place(name, *span),
            Expr::SelfKw(span) => self.variable_place("self", *span),
            Expr::Field {
                target,
                field,
                span,
            } => {
                let Some(mut place) = self.place_of(target)? else {
                    return Ok(None);
                };
                let TyKind::Adt(adt) = self.tk(place.ty) else {
                    return self.err(
                        Some(*span),
                        format!("`.{field}` on {}", self.show(place.ty)),
                    );
                };
                let def = self.module.types.adt(adt);
                let Some(index) = def.field_index(field) else {
                    return self.err(
                        Some(*span),
                        format!("`{}` has no field `{field}`", self.show(place.ty)),
                    );
                };
                let ty = def.struct_fields()[index as usize].ty;
                place.path.push((Proj::Field(index), ty));
                place.ty = ty;
                Ok(Some(place))
            }
            Expr::TupleIndex {
                target,
                index,
                span,
            } => {
                let Some(mut place) = self.place_of(target)? else {
                    return Ok(None);
                };
                let TyKind::Tuple(tys) = self.tk(place.ty) else {
                    return self.err(Some(*span), "tuple index on a non-tuple");
                };
                let Some(ty) = tys.get(*index).copied() else {
                    return self.err(Some(*span), "tuple index out of range");
                };
                place.path.push((Proj::Tuple(*index as u32), ty));
                place.ty = ty;
                Ok(Some(place))
            }
            Expr::Index {
                target,
                index,
                span,
            } => {
                let Some(mut place) = self.place_of(target)? else {
                    return Ok(None);
                };
                match self.tk(place.ty) {
                    TyKind::List(item) => {
                        let num = self.num_ty(NumKind::Num);
                        let i = self.expr(index, Some(num))?;
                        place.path.push((Proj::Index(i), item));
                        place.ty = item;
                    }
                    TyKind::Map(item) => {
                        let key = self.key_reg(index)?;
                        place.path.push((Proj::Key(key), item));
                        place.ty = item;
                    }
                    _ => {
                        return self
                            .err(Some(*span), format!("cannot index {}", self.show(place.ty)))
                    }
                }
                Ok(Some(place))
            }
            _ => Ok(None),
        }
    }

    fn variable_place(&mut self, name: &str, span: Span) -> LResult<Option<Place>> {
        if let Some(reg) = self.resolve_local(name) {
            if self.is_capture_reg(reg) {
                return self.err(
                    Some(span),
                    format!("`{name}` is captured by a lambda and cannot be changed there"),
                );
            }
            let ty = self.reg_ty(reg);
            return Ok(Some(Place {
                root: Root::Local(reg),
                path: Vec::new(),
                ty,
            }));
        }
        if let Some(global) = self.globals.get(name).copied() {
            let ty = self.module.globals[global.0 as usize].ty;
            return Ok(Some(Place {
                root: Root::Global(global),
                path: Vec::new(),
                ty,
            }));
        }
        Ok(None)
    }

    /// Copy of the value stored at `place`.
    pub fn read_place(&mut self, place: &Place) -> Reg {
        let mut cur = match place.root {
            Root::Local(reg) => reg,
            Root::Global(global) => {
                let ty = self.module.globals[global.0 as usize].ty;
                let dst = self.new_reg(ty);
                self.emit(Instr::GlobalGet { dst, global });
                dst
            }
        };
        if place.path.is_empty() {
            return match place.root {
                Root::Local(_) => self.mov(cur),
                Root::Global(_) => cur,
            };
        }
        for (proj, ty) in &place.path {
            let dst = self.new_reg(*ty);
            self.emit(Instr::Get {
                dst,
                base: cur,
                proj: *proj,
            });
            cur = dst;
        }
        cur
    }

    /// Takes the value at `path[..len]` out of the place so it can be changed
    /// in place; returns the register that holds it.
    fn enter(&mut self, place: &Place, len: usize) -> (Reg, Entered) {
        let mut entered = Entered {
            chain: Vec::new(),
            global: None,
        };
        let mut cur = match place.root {
            Root::Local(reg) => reg,
            Root::Global(global) => {
                let ty = self.module.globals[global.0 as usize].ty;
                let tmp = self.new_reg(ty);
                self.emit(Instr::GlobalGet { dst: tmp, global });
                entered.global = Some((global, tmp));
                tmp
            }
        };
        for (proj, ty) in &place.path[..len] {
            let taken = self.new_reg(*ty);
            self.emit(Instr::Take {
                dst: taken,
                base: cur,
                proj: *proj,
            });
            entered.chain.push((cur, *proj, taken));
            cur = taken;
        }
        (cur, entered)
    }

    fn leave(&mut self, entered: Entered) {
        for (parent, proj, taken) in entered.chain.into_iter().rev() {
            self.emit(Instr::Set {
                base: parent,
                proj,
                src: taken,
            });
        }
        if let Some((global, tmp)) = entered.global {
            self.emit(Instr::GlobalSet { global, src: tmp });
        }
    }

    /// Gives `f` the register holding the value at `place`; whatever `f`
    /// does to it is written back.
    pub fn with_place<R>(&mut self, place: &Place, f: impl FnOnce(&mut Self, Reg) -> R) -> R {
        let (leaf, entered) = self.enter(place, place.path.len());
        let out = f(self, leaf);
        self.leave(entered);
        out
    }

    pub fn store_place(&mut self, place: &Place, value: Reg) {
        match place.path.split_last() {
            None => match place.root {
                Root::Local(reg) => self.emit(Instr::Mov {
                    dst: reg,
                    src: value,
                }),
                Root::Global(global) => self.emit(Instr::GlobalSet { global, src: value }),
            },
            Some(((proj, _), _)) => {
                let (parent, entered) = self.enter(place, place.path.len() - 1);
                self.emit(Instr::Set {
                    base: parent,
                    proj: *proj,
                    src: value,
                });
                self.leave(entered);
            }
        }
    }

    pub fn is_capture_reg(&self, reg: Reg) -> bool {
        self.stack
            .last()
            .unwrap()
            .captures
            .iter()
            .any(|c| c.inner == reg)
    }
}

/// Does a `break` in this block leave the loop that owns it? Breaks inside
/// nested loops and lambdas leave those instead.
fn breaks(block: &Block) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Break(_) => true,
        Stmt::If { then, else_, .. } => breaks(then) || else_.as_ref().is_some_and(breaks),
        Stmt::Expr(e) => breaks_expr(e),
        Stmt::Let { init, .. } => match init {
            LetInit::Assign(e) | LetInit::Tint(e) => breaks_expr(e),
        },
        Stmt::Match { arms, .. } => arms.iter().any(|a| breaks_expr(&a.expr)),
        _ => false,
    })
}

fn breaks_expr(e: &Expr) -> bool {
    match e {
        Expr::If { then, else_, .. } => breaks(then) || breaks(else_),
        Expr::Block(inner, _) => breaks(inner),
        Expr::Match { arms, .. } => arms.iter().any(|a| breaks_expr(&a.expr)),
        Expr::Paren(inner, _) => breaks_expr(inner),
        _ => false,
    }
}
