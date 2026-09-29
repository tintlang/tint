//! Calls: functions, closures, constructors, and the built-in methods of
//! strings, lists, maps, `Option`, `Result` and `Vec2`.

use super::analysis::builtin_mutator;
use super::*;
use tint_ast::Expr;

const BUILTIN_ENUMS: [(&str, [&str; 2]); 2] = [("Option", ["None", "Some"]), ("Result", ["Ok", "Err"])];

impl<'a> Lowerer<'a> {
    fn has_variant(&self, enum_name: &str, variant: &str) -> bool {
        if let Some((_, variants)) = BUILTIN_ENUMS.iter().find(|(n, _)| *n == enum_name) {
            return variants.contains(&variant);
        }
        self.enums
            .get(enum_name)
            .is_some_and(|e| e.variants.iter().any(|v| v.name == variant))
    }

    /// The enum a bare variant name belongs to, if exactly one enum has it.
    fn owner_of_variant(&self, variant: &str) -> Option<String> {
        let mut owners: Vec<String> = Vec::new();
        for (name, variants) in BUILTIN_ENUMS {
            if variants.contains(&variant) {
                owners.push(name.to_string());
            }
        }
        for (name, info) in &self.enums {
            if info.variants.iter().any(|v| v.name == variant) {
                owners.push(name.clone());
            }
        }
        if owners.len() == 1 {
            owners.pop()
        } else {
            None
        }
    }

    pub fn call(&mut self, call: &Expr, target: &Expr, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        // Enum constructors: `Enum::Variant(..)` and a bare `Some(..)`.
        let variant = match target {
            Expr::Namespace { base, item, .. } => match base.as_ref() {
                Expr::Ident(enum_name, _) if self.has_variant(enum_name, item) => {
                    Some((enum_name.clone(), item.clone()))
                }
                _ => None,
            },
            Expr::Ident(name, _)
                if self.resolve_local(name).is_none()
                    && !self.globals.contains_key(name)
                    && !self.fns.contains_key(name) =>
            {
                self.owner_of_variant(name).map(|owner| (owner, name.clone()))
            }
            _ => None,
        };
        if let Some((enum_name, variant)) = variant {
            return self.variant_call(call, &enum_name, &variant, args, hint, span);
        }

        if let Expr::Field { target: receiver, field: method, span: field_span } = target {
            return self.method_call(call, receiver, method, args, hint, *field_span);
        }

        if let Expr::Ident(name, _) = target {
            if let Some(callee) = self.resolve_local(name) {
                return self.call_closure_exprs(callee, args, span);
            }
            if let Some(global) = self.globals.get(name).copied() {
                let ty = self.module.globals[global.0 as usize].ty;
                let callee = self.new_reg(ty);
                self.emit(Instr::GlobalGet { dst: callee, global });
                return self.call_closure_exprs(callee, args, span);
            }
            let info = match self.fns.get(name).cloned() {
                Some(info) => Some(info),
                None if self.generic_fns.contains_key(name) => {
                    let key = target as *const Expr as usize;
                    let Some(types) = self.model.instances.get(&key).cloned() else {
                        return self.err(Some(span), format!("no type arguments were inferred for this call of `{name}`"));
                    };
                    let targs = types.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
                    Some(self.fn_instance(name, targs, span)?)
                }
                None => None,
            };
            if let Some(info) = info {
                let slots = match tint_ast::bind_call_args(name, &info.sigs, args) {
                    Ok(slots) => slots,
                    Err(message) => return self.err(Some(span), message),
                };
                // Arguments are evaluated in the order they are written; the
                // registers are then put in parameter order.
                let mut given: Vec<(usize, usize)> = slots.iter().enumerate().filter_map(|(p, a)| a.map(|a| (a, p))).collect();
                given.sort();
                let mut regs: Vec<Option<Reg>> = vec![None; info.params.len()];
                for (arg_index, param) in given {
                    regs[param] = Some(self.expr_as(tint_ast::arg_value(&args[arg_index]), info.params[param])?);
                }
                let mut ordered = Vec::with_capacity(regs.len());
                for (param, reg) in regs.into_iter().enumerate() {
                    ordered.push(match reg {
                        Some(reg) => reg,
                        None => {
                            let default = info.defaults[param].expect("bind_call_args checked the default");
                            self.default_arg(default, info.params[param])?
                        }
                    });
                }
                let regs = ordered;
                let dst = self.new_reg(info.ret);
                self.emit(Instr::Call { dst, func: info.id, args: regs });
                return Ok(dst);
            }
            if let Some(result) = self.builtin_call(name, args, span)? {
                return Ok(result);
            }
            return self.err(Some(span), format!("call of `{name}`, which is not a function that can be lowered"));
        }

        let callee = self.expr(target, None)?;
        self.call_closure_exprs(callee, args, span)
    }

    /// Lowers a parameter default at the call site. Only globals are visible
    /// to it, never the caller's locals.
    fn default_arg(&mut self, default: &Expr, ty: TyId) -> LResult<Reg> {
        let hidden: Vec<_> = self
            .stack
            .iter_mut()
            .map(|f| std::mem::replace(&mut f.scopes, vec![Default::default()]))
            .collect();
        let result = self.expr_as(default, ty);
        for (f, scopes) in self.stack.iter_mut().zip(hidden) {
            f.scopes = scopes;
        }
        result
    }

    fn variant_call(&mut self, call: &Expr, enum_name: &str, variant: &str, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let ty = self.pick(call, hint)?;
        let TyKind::Adt(adt) = self.tk(ty) else {
            return self.err(Some(span), format!("`{enum_name}` is not an enum"));
        };
        let def = self.module.types.adt(adt);
        let Some(index) = def.variant_index(variant) else {
            return self.err(Some(span), format!("`{enum_name}` has no variant `{variant}`"));
        };
        let fields = def.variants()[index as usize].fields.clone();
        if fields.len() != args.len() {
            return self.err(Some(span), format!("`{enum_name}::{variant}` takes {} values, got {}", fields.len(), args.len()));
        }
        let mut regs = Vec::new();
        for (arg, field) in args.iter().zip(&fields) {
            regs.push(self.expr_as(arg, field.ty)?);
        }
        let dst = self.new_reg(ty);
        self.emit(Instr::Variant { dst, adt, variant: index, fields: regs });
        Ok(dst)
    }

    pub fn call_closure_exprs(&mut self, callee: Reg, args: &[Expr], span: Span) -> LResult<Reg> {
        let TyKind::Fn(params, _) = self.tk(self.reg_ty(callee)) else {
            return self.err(Some(span), format!("call of {}, which is not a function", self.show(self.reg_ty(callee))));
        };
        if params.len() != args.len() {
            return self.err(Some(span), format!("function takes {} arguments, got {}", params.len(), args.len()));
        }
        let mut regs = Vec::new();
        for (arg, ty) in args.iter().zip(&params) {
            regs.push(self.expr_as(arg, *ty)?);
        }
        self.call_closure_regs(callee, regs, span)
    }

    /// Calls a function value, converting numbers to the parameter types.
    pub fn call_closure_regs(&mut self, callee: Reg, args: Vec<Reg>, span: Span) -> LResult<Reg> {
        let TyKind::Fn(params, ret) = self.tk(self.reg_ty(callee)) else {
            return self.err(Some(span), "call of a non-function");
        };
        if params.len() != args.len() {
            return self.err(Some(span), format!("function takes {} arguments, got {}", params.len(), args.len()));
        }
        let mut regs = Vec::new();
        for (arg, ty) in args.into_iter().zip(params) {
            regs.push(self.coerce(arg, ty, Some(span))?);
        }
        let dst = self.new_reg(ret);
        self.emit(Instr::CallClosure { dst, callee, args: regs });
        Ok(dst)
    }

    fn builtin_call(&mut self, name: &str, args: &[Expr], span: Span) -> LResult<Option<Reg>> {
        let num = self.num_ty(NumKind::Num);
        let arity = |this: &Self, n: usize| -> LResult<()> {
            if args.len() == n {
                Ok(())
            } else {
                this.err(Some(span), format!("`{name}` takes {n} arguments, got {}", args.len()))
            }
        };
        let (f, n) = match name {
            "sqrt" => (RtFn::Sqrt, 1),
            "abs" => (RtFn::Abs, 1),
            "sign" => (RtFn::Sign, 1),
            "min" => (RtFn::Min, 2),
            "max" => (RtFn::Max, 2),
            "clamp" => (RtFn::Clamp, 3),
            "vec2" => {
                arity(self, 2)?;
                let x = self.expr_as(&args[0], num)?;
                let y = self.expr_as(&args[1], num)?;
                let adt = self.adt_instance("Vec2", Vec::new())?;
                let ty = self.module.types.adt_ty(adt);
                let dst = self.new_reg(ty);
                self.emit(Instr::Struct { dst, adt, fields: vec![x, y] });
                return Ok(Some(dst));
            }
            "parse_number" => {
                arity(self, 1)?;
                let text_ty = self.str_ty();
                let text = self.expr_as(&args[0], text_ty)?;
                let adt = self.adt_instance("Result", vec![num, text_ty])?;
                let ty = self.module.types.adt_ty(adt);
                return Ok(Some(self.rt(RtFn::ParseNumber, vec![text], ty)));
            }
            "print" | "println" | "log" | "dbg" | "debug" | "error" => {
                let f = match name {
                    "dbg" | "debug" => HostFn::Dbg,
                    "error" => HostFn::Error,
                    _ => HostFn::Print,
                };
                let mut regs = Vec::new();
                for arg in args {
                    regs.push(self.expr(arg, None)?);
                }
                let ty = self.unit_ty();
                let dst = self.new_reg(ty);
                self.emit(Instr::Host { dst, f, args: regs });
                return Ok(Some(dst));
            }
            "read_line" | "read_key" => {
                arity(self, 0)?;
                let f = if name == "read_line" { HostFn::ReadLine } else { HostFn::ReadKey };
                let ty = self.str_ty();
                let dst = self.new_reg(ty);
                self.emit(Instr::Host { dst, f, args: Vec::new() });
                return Ok(Some(dst));
            }
            _ => return Ok(None),
        };
        arity(self, n)?;
        let mut regs = Vec::new();
        for arg in args {
            regs.push(self.expr_as(arg, num)?);
        }
        Ok(Some(self.rt(f, regs, num)))
    }

    // ------------------------------------------------------------ methods

    fn method_call(&mut self, call: &Expr, recv: &Expr, method: &str, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let recv_ty = self.model.type_of(recv).cloned();
        if let Some(ty) = &recv_ty {
            if builtin_mutator(ty, method) {
                return self.mutating_builtin(recv, method, args, span);
            }
            if let Type::Struct(name) = ty {
                if let Some(info) = self.methods.get(&(name.clone(), method.to_string())).cloned() {
                    return self.user_method(recv, method, args, info, span);
                }
            }
        }
        let r = self.expr_ref(recv)?;
        let r_ty = self.reg_ty(r);
        match self.tk(r_ty) {
            TyKind::Str => self.string_method(r, method, args, span),
            TyKind::List(elem) => self.list_method(r, elem, method, args, hint, span),
            TyKind::Map(value) => self.map_method(r, value, method, args, span),
            TyKind::Adt(adt) => {
                let base = self.module.types.adt(adt).base.clone();
                match base.as_str() {
                    "Option" | "Result" => self.option_method(call, r, adt, method, args, hint, span),
                    "Vec2" => self.vec2_method(r, method, args, span),
                    _ => {
                        // A field that holds a function.
                        let def = self.module.types.adt(adt);
                        match def.field_index(method) {
                            Some(index) => {
                                let fty = def.struct_fields()[index as usize].ty;
                                let f = self.new_reg(fty);
                                self.emit(Instr::Get { dst: f, base: r, proj: Proj::Field(index) });
                                self.call_closure_exprs(f, args, span)
                            }
                            None => self.err(Some(span), format!("`{}` has no method `{method}`", self.show(r_ty))),
                        }
                    }
                }
            }
            _ => self.err(Some(span), format!("no method `{method}` on {}", self.show(r_ty))),
        }
    }

    fn user_method(&mut self, recv: &Expr, method: &str, args: &[Expr], info: MethodInfo, span: Span) -> LResult<Reg> {
        if info.params.len() != args.len() {
            return self.err(Some(span), format!("`{method}` takes {} arguments, got {}", info.params.len(), args.len()));
        }
        let place = if info.inout_self { self.place_of(recv)? } else { None };
        let me = match &place {
            Some(p) => self.read_place(p),
            None => self.expr(recv, None)?,
        };
        let mut regs = vec![me];
        for (arg, ty) in args.iter().zip(&info.params) {
            regs.push(self.expr_as(arg, *ty)?);
        }
        if !info.inout_self {
            let dst = self.new_reg(info.ret);
            self.emit(Instr::Call { dst, func: info.id, args: regs });
            return Ok(dst);
        }
        let pair_ty = self.module.funcs[info.id.0 as usize].ret;
        let me_ty = self.reg_ty(me);
        let pair = self.new_reg(pair_ty);
        self.emit(Instr::Call { dst: pair, func: info.id, args: regs });
        let result = self.new_reg(info.ret);
        self.emit(Instr::Get { dst: result, base: pair, proj: Proj::Tuple(0) });
        if let Some(p) = &place {
            let updated = self.new_reg(me_ty);
            self.emit(Instr::Get { dst: updated, base: pair, proj: Proj::Tuple(1) });
            self.store_place(p, updated);
        }
        Ok(result)
    }

    fn arity(&self, method: &str, args: &[Expr], allowed: &[usize], span: Span) -> LResult<()> {
        if allowed.contains(&args.len()) {
            Ok(())
        } else {
            self.err(Some(span), format!("`.{method}` takes {allowed:?} arguments, got {}", args.len()))
        }
    }

    fn string_method(&mut self, r: Reg, method: &str, args: &[Expr], span: Span) -> LResult<Reg> {
        let (str_ty, num, bool_ty) = (self.str_ty(), self.num_ty(NumKind::Num), self.bool_ty());
        let (f, arg_tys, ret): (RtFn, Vec<TyId>, TyId) = match method {
            "len" => (RtFn::StrLen, vec![], num),
            "is_empty" => (RtFn::StrIsEmpty, vec![], bool_ty),
            "trim" => (RtFn::StrTrim, vec![], str_ty),
            "to_upper" => (RtFn::StrUpper, vec![], str_ty),
            "to_lower" => (RtFn::StrLower, vec![], str_ty),
            "contains" => (RtFn::StrContains, vec![str_ty], bool_ty),
            "starts_with" => (RtFn::StrStartsWith, vec![str_ty], bool_ty),
            "ends_with" => (RtFn::StrEndsWith, vec![str_ty], bool_ty),
            "split" => {
                let list = self.module.types.list(str_ty);
                (RtFn::StrSplit, vec![str_ty], list)
            }
            "replace" => (RtFn::StrReplace, vec![str_ty, str_ty], str_ty),
            "slice" => {
                self.arity(method, args, &[1, 2], span)?;
                (RtFn::StrSlice, vec![num; args.len()], str_ty)
            }
            _ => return self.err(Some(span), format!("String has no method `{method}`")),
        };
        self.arity(method, args, &[arg_tys.len()], span)?;
        let mut regs = vec![r];
        for (arg, ty) in args.iter().zip(arg_tys) {
            regs.push(self.expr_as(arg, ty)?);
        }
        Ok(self.rt(f, regs, ret))
    }

    fn list_method(&mut self, r: Reg, elem: TyId, method: &str, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let (str_ty, num, bool_ty) = (self.str_ty(), self.num_ty(NumKind::Num), self.bool_ty());
        let list_ty = self.reg_ty(r);
        let (f, arg_tys, ret): (RtFn, Vec<TyId>, TyId) = match method {
            "len" => (RtFn::ListLen, vec![], num),
            "is_empty" => (RtFn::ListIsEmpty, vec![], bool_ty),
            "contains" => (RtFn::ListContains, vec![elem], bool_ty),
            "join" => (RtFn::ListJoin, vec![str_ty], str_ty),
            "reverse" => (RtFn::ListReverse, vec![], list_ty),
            "sort" => {
                if !matches!(self.tk(elem), TyKind::Num(_) | TyKind::Str) {
                    return self.err(Some(span), "List.sort expects only numbers or only strings");
                }
                (RtFn::ListSort, vec![], list_ty)
            }
            "slice" => {
                self.arity(method, args, &[1, 2], span)?;
                (RtFn::ListSlice, vec![num; args.len()], list_ty)
            }
            "map" => return self.list_map(r, elem, args, hint, span),
            "filter" => return self.list_filter(r, elem, args, span),
            "find" => return self.list_find(r, elem, args, span),
            _ => return self.err(Some(span), format!("List has no method `{method}`")),
        };
        self.arity(method, args, &[arg_tys.len()], span)?;
        let mut regs = vec![r];
        for (arg, ty) in args.iter().zip(arg_tys) {
            regs.push(self.expr_as(arg, ty)?);
        }
        Ok(self.rt(f, regs, ret))
    }

    fn map_method(&mut self, r: Reg, value: TyId, method: &str, args: &[Expr], span: Span) -> LResult<Reg> {
        let (str_ty, num, bool_ty) = (self.str_ty(), self.num_ty(NumKind::Num), self.bool_ty());
        let option = self.adt_instance("Option", vec![value])?;
        let option_ty = self.module.types.adt_ty(option);
        let keys = self.module.types.list(str_ty);
        let values = self.module.types.list(value);
        let (f, ret): (RtFn, TyId) = match method {
            "len" => (RtFn::MapLen, num),
            "is_empty" => (RtFn::MapIsEmpty, bool_ty),
            "has" => (RtFn::MapHas, bool_ty),
            "get" => (RtFn::MapGet, option_ty),
            "keys" => (RtFn::MapKeys, keys),
            "values" => (RtFn::MapValues, values),
            _ => return self.err(Some(span), format!("Map has no method `{method}`")),
        };
        let want = if matches!(method, "has" | "get") { 1 } else { 0 };
        self.arity(method, args, &[want], span)?;
        let mut regs = vec![r];
        for arg in args {
            regs.push(self.key_reg(arg)?);
        }
        Ok(self.rt(f, regs, ret))
    }

    /// `push`, `pop`, `remove` on lists and `set`, `remove` on maps: they
    /// change the variable they are called on.
    fn mutating_builtin(&mut self, recv: &Expr, method: &str, args: &[Expr], span: Span) -> LResult<Reg> {
        let place = self.place_of(recv)?;
        let recv_ty = match &place {
            Some(p) => p.ty,
            None => self.nat(recv)?,
        };
        let (str_ty, num) = (self.str_ty(), self.num_ty(NumKind::Num));
        let unit = self.unit_ty();
        let mut regs: Vec<Reg> = Vec::new();
        let (f, ret) = match (self.tk(recv_ty), method) {
            (TyKind::List(elem), "push") => {
                self.arity(method, args, &[1], span)?;
                regs.push(self.expr_as(&args[0], elem)?);
                (RtFn::ListPush, unit)
            }
            (TyKind::List(elem), "pop") => {
                self.arity(method, args, &[0], span)?;
                let option = self.adt_instance("Option", vec![elem])?;
                (RtFn::ListPop, self.module.types.adt_ty(option))
            }
            (TyKind::List(elem), "remove") => {
                self.arity(method, args, &[1], span)?;
                regs.push(self.expr_as(&args[0], num)?);
                (RtFn::ListRemove, elem)
            }
            (TyKind::Map(value), "set") => {
                self.arity(method, args, &[2], span)?;
                regs.push(self.key_reg(&args[0])?);
                regs.push(self.expr_as(&args[1], value)?);
                (RtFn::MapSet, unit)
            }
            (TyKind::Map(value), "remove") => {
                self.arity(method, args, &[1], span)?;
                regs.push(self.key_reg(&args[0])?);
                let option = self.adt_instance("Option", vec![value])?;
                (RtFn::MapRemove, self.module.types.adt_ty(option))
            }
            _ => return self.err(Some(span), format!("`.{method}` on {}", self.show(recv_ty))),
        };
        let _ = str_ty;
        let dst = self.new_reg(ret);
        match place {
            Some(p) => self.with_place(&p, |this, leaf| {
                let mut all = vec![leaf];
                all.extend(regs);
                this.emit(Instr::Rt { dst, f, args: all });
            }),
            None => {
                let leaf = self.expr(recv, None)?;
                let mut all = vec![leaf];
                all.extend(regs);
                self.emit(Instr::Rt { dst, f, args: all });
            }
        }
        Ok(dst)
    }

    // ------------------------------------------------ list iteration

    /// Runs `body` once per element of `list`. `body` gets the element and the
    /// block after the loop, which it may jump to.
    fn list_loop(&mut self, list: Reg, mut body: impl FnMut(&mut Self, Reg, BlockId) -> LResult<()>) -> LResult<()> {
        let TyKind::List(elem) = self.tk(self.reg_ty(list)) else {
            return self.err(None, "iteration over a non-list");
        };
        let num = self.num_ty(NumKind::Num);
        let bool_ty = self.bool_ty();
        let n = self.rt(RtFn::ListLen, vec![list], num);
        let i = self.const_reg(num, Const::Float(0.0));
        let (head, body_b, exit) = (self.new_block(), self.new_block(), self.new_block());
        self.terminate(Term::Jump(head));
        self.switch_to(head);
        let more = self.new_reg(bool_ty);
        self.emit(Instr::Cmp { dst: more, op: CmpOp::Lt, a: i, b: n });
        self.terminate(Term::Branch { cond: more, then_: body_b, else_: exit });
        self.switch_to(body_b);
        let item = self.new_reg(elem);
        self.emit(Instr::Get { dst: item, base: list, proj: Proj::Index(i) });
        body(self, item, exit)?;
        let one = self.const_reg(num, Const::Float(1.0));
        let next = self.arith(BinOp::Add, NumKind::Num, num, i, one);
        self.emit(Instr::Mov { dst: i, src: next });
        self.terminate(Term::Jump(head));
        self.switch_to(exit);
        Ok(())
    }

    fn function_arg(&mut self, args: &[Expr], method: &str, span: Span) -> LResult<Reg> {
        self.arity(method, args, &[1], span)?;
        let f = self.expr(&args[0], None)?;
        if !matches!(self.tk(self.reg_ty(f)), TyKind::Fn(..)) {
            return self.err(Some(span), format!("`.{method}` needs a function"));
        }
        Ok(f)
    }

    fn list_map(&mut self, list: Reg, elem: TyId, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let _ = elem;
        let f = self.function_arg(args, "map", span)?;
        let TyKind::Fn(_, ret) = self.tk(self.reg_ty(f)) else { unreachable!() };
        let out_elem = match hint.map(|h| self.tk(h)) {
            Some(TyKind::List(h)) if self.compat(ret, h) => h,
            _ => ret,
        };
        let out_ty = self.module.types.list(out_elem);
        let out = self.new_reg(out_ty);
        self.emit(Instr::List { dst: out, items: Vec::new() });
        let unit = self.unit_ty();
        self.list_loop(list, |this, item, _| {
            let mapped = this.call_closure_regs(f, vec![item], span)?;
            let mapped = this.coerce(mapped, out_elem, Some(span))?;
            this.rt(RtFn::ListPush, vec![out, mapped], unit);
            Ok(())
        })?;
        Ok(out)
    }

    fn list_filter(&mut self, list: Reg, elem: TyId, args: &[Expr], span: Span) -> LResult<Reg> {
        let f = self.function_arg(args, "filter", span)?;
        let out_ty = self.reg_ty(list);
        let out = self.new_reg(out_ty);
        self.emit(Instr::List { dst: out, items: Vec::new() });
        let unit = self.unit_ty();
        let bool_ty = self.bool_ty();
        self.list_loop(list, |this, item, _| {
            let keep = this.call_closure_regs(f, vec![item], span)?;
            let keep = this.coerce(keep, bool_ty, Some(span))?;
            let (yes, join) = (this.new_block(), this.new_block());
            this.terminate(Term::Branch { cond: keep, then_: yes, else_: join });
            this.switch_to(yes);
            let item = this.coerce(item, elem, Some(span))?;
            this.rt(RtFn::ListPush, vec![out, item], unit);
            this.terminate(Term::Jump(join));
            this.switch_to(join);
            Ok(())
        })?;
        Ok(out)
    }

    fn list_find(&mut self, list: Reg, elem: TyId, args: &[Expr], span: Span) -> LResult<Reg> {
        let f = self.function_arg(args, "find", span)?;
        let option = self.adt_instance("Option", vec![elem])?;
        let option_ty = self.module.types.adt_ty(option);
        let none = self.module.types.adt(option).variant_index("None").unwrap();
        let some = self.module.types.adt(option).variant_index("Some").unwrap();
        let result = self.new_reg(option_ty);
        self.emit(Instr::Variant { dst: result, adt: option, variant: none, fields: Vec::new() });
        let bool_ty = self.bool_ty();
        self.list_loop(list, |this, item, exit| {
            let hit = this.call_closure_regs(f, vec![item], span)?;
            let hit = this.coerce(hit, bool_ty, Some(span))?;
            let (yes, no) = (this.new_block(), this.new_block());
            this.terminate(Term::Branch { cond: hit, then_: yes, else_: no });
            this.switch_to(yes);
            this.emit(Instr::Variant { dst: result, adt: option, variant: some, fields: vec![item] });
            this.terminate(Term::Jump(exit));
            this.switch_to(no);
            Ok(())
        })?;
        Ok(result)
    }

    // ------------------------------------------------ Option / Result

    /// Switches on the tag of `r`: returns the tag and the blocks for
    /// "variant == index" and "anything else". Leaves the builder in an
    /// unspecified block; callers `switch_to` one of the two.
    fn split_on(&mut self, r: Reg, index: i64) -> (Reg, BlockId, BlockId) {
        let tag_ty = self.num_ty(NumKind::I64);
        let tag = self.new_reg(tag_ty);
        self.emit(Instr::Tag { dst: tag, src: r });
        let (hit, miss) = (self.new_block(), self.new_block());
        self.terminate(Term::Switch { value: tag, cases: vec![(index, hit)], default: miss });
        (tag, hit, miss)
    }

    /// Arguments a fallback closure gets in the failure branch: the error of
    /// a `Result`, nothing for an `Option`. Emitted in the current block.
    fn failure_payload(&mut self, r: Reg, def: &AdtDef, is_option: bool) -> Vec<Reg> {
        if is_option {
            return Vec::new();
        }
        let err_ty = def.variants()[1].fields[0].ty;
        let e = self.new_reg(err_ty);
        self.emit(Instr::Payload { dst: e, src: r, variant: 1, index: 0 });
        vec![e]
    }

    #[allow(clippy::too_many_arguments)]
    fn option_method(&mut self, call: &Expr, r: Reg, adt: AdtId, method: &str, args: &[Expr], hint: Option<TyId>, span: Span) -> LResult<Reg> {
        let def = self.module.types.adt(adt).clone();
        let is_option = def.base == "Option";
        let ok_index: u32 = if is_option { 1 } else { 0 };
        let payload_ty = def.variants()[ok_index as usize].fields[0].ty;
        let bool_ty = self.bool_ty();
        let tag_ty = self.num_ty(NumKind::I64);

        match method {
            "is_some" | "is_none" | "is_ok" | "is_err" => {
                self.arity(method, args, &[0], span)?;
                let applies = match method {
                    "is_some" | "is_none" => is_option,
                    _ => !is_option,
                };
                let wants_ok = matches!(method, "is_some" | "is_ok");
                if !applies {
                    return Ok(self.const_reg(bool_ty, Const::Bool(false)));
                }
                let tag = self.new_reg(tag_ty);
                self.emit(Instr::Tag { dst: tag, src: r });
                let want = if wants_ok { ok_index } else { 1 - ok_index };
                let c = self.const_reg(tag_ty, Const::Int(want as i64));
                let dst = self.new_reg(bool_ty);
                self.emit(Instr::Cmp { dst, op: CmpOp::Eq, a: tag, b: c });
                Ok(dst)
            }
            "unwrap" | "expect" => {
                self.arity(method, args, &[if method == "expect" { 1 } else { 0 }], span)?;
                for arg in args {
                    self.expr(arg, None)?;
                }
                let tag = self.new_reg(tag_ty);
                self.emit(Instr::Tag { dst: tag, src: r });
                let (ok, fail) = (self.new_block(), self.new_block());
                self.terminate(Term::Switch { value: tag, cases: vec![(ok_index as i64, ok)], default: fail });
                self.switch_to(fail);
                self.terminate(Term::Trap(format!("{method} called on an empty or failed value")));
                self.switch_to(ok);
                let dst = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst, src: r, variant: ok_index, index: 0 });
                Ok(dst)
            }
            "unwrap_or" => {
                self.arity(method, args, &[1], span)?;
                let fallback = self.expr_as(&args[0], payload_ty)?;
                let tag = self.new_reg(tag_ty);
                self.emit(Instr::Tag { dst: tag, src: r });
                let result = self.new_reg(payload_ty);
                let (ok, other, join) = (self.new_block(), self.new_block(), self.new_block());
                self.terminate(Term::Switch { value: tag, cases: vec![(ok_index as i64, ok)], default: other });
                self.switch_to(ok);
                self.emit(Instr::Payload { dst: result, src: r, variant: ok_index, index: 0 });
                self.terminate(Term::Jump(join));
                self.switch_to(other);
                self.emit(Instr::Mov { dst: result, src: fallback });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "map" | "and_then" => {
                let f = self.function_arg(args, method, span)?;
                let TyKind::Fn(_, f_ret) = self.tk(self.reg_ty(f)) else { unreachable!() };
                // The type of the whole result.
                let result_ty = if method == "and_then" {
                    f_ret
                } else {
                    let mut new_args = def.args.clone();
                    new_args[0] = f_ret;
                    let adt2 = self.adt_instance(&def.base, new_args)?;
                    self.module.types.adt_ty(adt2)
                };
                let result_ty = match hint {
                    Some(h) if self.compat(result_ty, h) && method == "map" => h,
                    _ => result_ty,
                };
                let TyKind::Adt(result_adt) = self.tk(result_ty) else {
                    return self.err(Some(span), format!("`.{method}` must return an {}", def.base));
                };
                let result_def = self.module.types.adt(result_adt).clone();
                if result_def.base != def.base {
                    return self.err(Some(span), format!("`.{method}` must return an {}", def.base));
                }
                let _ = call;
                let tag = self.new_reg(tag_ty);
                self.emit(Instr::Tag { dst: tag, src: r });
                let result = self.new_reg(result_ty);
                let (ok, fail, join) = (self.new_block(), self.new_block(), self.new_block());
                self.terminate(Term::Switch { value: tag, cases: vec![(ok_index as i64, ok)], default: fail });
                self.switch_to(ok);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: ok_index, index: 0 });
                let out = self.call_closure_regs(f, vec![payload], span)?;
                if method == "and_then" {
                    let out = self.coerce(out, result_ty, Some(span))?;
                    self.emit(Instr::Mov { dst: result, src: out });
                } else {
                    let want = result_def.variants()[ok_index as usize].fields[0].ty;
                    let out = self.coerce(out, want, Some(span))?;
                    self.emit(Instr::Variant { dst: result, adt: result_adt, variant: ok_index, fields: vec![out] });
                }
                self.terminate(Term::Jump(join));
                self.switch_to(fail);
                if is_option {
                    self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 0, fields: Vec::new() });
                } else {
                    let err_ty = def.variants()[1].fields[0].ty;
                    let want = result_def.variants()[1].fields[0].ty;
                    let e = self.new_reg(err_ty);
                    self.emit(Instr::Payload { dst: e, src: r, variant: 1, index: 0 });
                    let e = self.coerce(e, want, Some(span))?;
                    self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 1, fields: vec![e] });
                }
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "unwrap_err" | "expect_err" if !is_option => {
                self.arity(method, args, &[if method == "expect_err" { 1 } else { 0 }], span)?;
                for arg in args {
                    self.expr(arg, None)?;
                }
                let (_, ok, fail) = self.split_on(r, 1);
                self.switch_to(fail);
                self.terminate(Term::Trap(format!("{method} called on an Ok value")));
                self.switch_to(ok);
                let err_ty = def.variants()[1].fields[0].ty;
                let dst = self.new_reg(err_ty);
                self.emit(Instr::Payload { dst, src: r, variant: 1, index: 0 });
                Ok(dst)
            }
            "is_some_and" | "is_ok_and" | "is_err_and" => {
                let want = if method == "is_err_and" { 1 } else { ok_index };
                if (method == "is_some_and") != is_option || (is_option && method != "is_some_and") {
                    return self.err(Some(span), format!("{} has no method `{method}`", def.base));
                }
                let f = self.function_arg(args, method, span)?;
                let result = self.new_reg(bool_ty);
                let (_, yes, no) = self.split_on(r, want as i64);
                let join = self.new_block();
                self.switch_to(yes);
                let inner_ty = def.variants()[want as usize].fields[0].ty;
                let payload = self.new_reg(inner_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: want, index: 0 });
                let out = self.call_closure_regs(f, vec![payload], span)?;
                let out = self.coerce(out, bool_ty, Some(span))?;
                self.emit(Instr::Mov { dst: result, src: out });
                self.terminate(Term::Jump(join));
                self.switch_to(no);
                let no_value = self.const_reg(bool_ty, Const::Bool(false));
                self.emit(Instr::Mov { dst: result, src: no_value });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "unwrap_or_else" => {
                let f = self.function_arg(args, method, span)?;
                let result = self.new_reg(payload_ty);
                let (_, ok, other) = self.split_on(r, ok_index as i64);
                let join = self.new_block();
                self.switch_to(ok);
                self.emit(Instr::Payload { dst: result, src: r, variant: ok_index, index: 0 });
                self.terminate(Term::Jump(join));
                self.switch_to(other);
                let fallback_args = self.failure_payload(r, &def, is_option);
                let out = self.call_closure_regs(f, fallback_args, span)?;
                let out = self.coerce(out, payload_ty, Some(span))?;
                self.emit(Instr::Mov { dst: result, src: out });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "map_or" => {
                self.arity(method, args, &[2], span)?;
                let default = self.expr(&args[0], hint)?;
                let f = self.expr(&args[1], None)?;
                let TyKind::Fn(_, out_ty) = self.tk(self.reg_ty(f)) else {
                    return self.err(Some(span), "`.map_or` needs a function");
                };
                let default = self.coerce(default, out_ty, Some(span))?;
                let result = self.new_reg(out_ty);
                let (_, ok, other) = self.split_on(r, ok_index as i64);
                let join = self.new_block();
                self.switch_to(ok);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: ok_index, index: 0 });
                let out = self.call_closure_regs(f, vec![payload], span)?;
                let out = self.coerce(out, out_ty, Some(span))?;
                self.emit(Instr::Mov { dst: result, src: out });
                self.terminate(Term::Jump(join));
                self.switch_to(other);
                self.emit(Instr::Mov { dst: result, src: default });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "filter" if is_option => {
                let f = self.function_arg(args, method, span)?;
                let r_ty = self.reg_ty(r);
                let result = self.new_reg(r_ty);
                let (_, some, none) = self.split_on(r, ok_index as i64);
                let join = self.new_block();
                self.switch_to(some);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: ok_index, index: 0 });
                let keep = self.call_closure_regs(f, vec![payload], span)?;
                let keep = self.coerce(keep, bool_ty, Some(span))?;
                let (yes, no) = (self.new_block(), self.new_block());
                self.terminate(Term::Branch { cond: keep, then_: yes, else_: no });
                self.switch_to(yes);
                self.emit(Instr::Mov { dst: result, src: r });
                self.terminate(Term::Jump(join));
                self.switch_to(no);
                self.emit(Instr::Variant { dst: result, adt, variant: 0, fields: Vec::new() });
                self.terminate(Term::Jump(join));
                self.switch_to(none);
                self.emit(Instr::Variant { dst: result, adt, variant: 0, fields: Vec::new() });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "or" | "or_else" => {
                let other = if method == "or" {
                    self.arity(method, args, &[1], span)?;
                    let hint_ty = if is_option { Some(self.reg_ty(r)) } else { hint };
                    Some(self.expr(&args[0], hint_ty)?)
                } else {
                    None
                };
                let f = if method == "or_else" { Some(self.function_arg(args, method, span)?) } else { None };
                let result_ty = match (other, f) {
                    (Some(o), _) => self.reg_ty(o),
                    (_, Some(f)) => match self.tk(self.reg_ty(f)) {
                        TyKind::Fn(_, ret) => ret,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                };
                let TyKind::Adt(result_adt) = self.tk(result_ty) else {
                    return self.err(Some(span), format!("`.{method}` must produce an {}", def.base));
                };
                let result_def = self.module.types.adt(result_adt).clone();
                if result_def.base != def.base {
                    return self.err(Some(span), format!("`.{method}` must produce an {}", def.base));
                }
                let result = self.new_reg(result_ty);
                let (_, ok, other_block) = self.split_on(r, ok_index as i64);
                let join = self.new_block();
                self.switch_to(ok);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: ok_index, index: 0 });
                let want = result_def.variants()[ok_index as usize].fields[0].ty;
                let payload = self.coerce(payload, want, Some(span))?;
                self.emit(Instr::Variant { dst: result, adt: result_adt, variant: ok_index, fields: vec![payload] });
                self.terminate(Term::Jump(join));
                self.switch_to(other_block);
                let out = match (other, f) {
                    (Some(o), _) => o,
                    (_, Some(f)) => {
                        let fallback_args = self.failure_payload(r, &def, is_option);
                        self.call_closure_regs(f, fallback_args, span)?
                    }
                    _ => unreachable!(),
                };
                let out = self.coerce(out, result_ty, Some(span))?;
                self.emit(Instr::Mov { dst: result, src: out });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "ok_or" | "ok_or_else" if is_option => {
                let f = if method == "ok_or_else" { Some(self.function_arg(args, method, span)?) } else { None };
                let err_value = if method == "ok_or" {
                    self.arity(method, args, &[1], span)?;
                    let err_hint = match hint.map(|h| self.tk(h)) {
                        Some(TyKind::Adt(h)) if self.module.types.adt(h).base == "Result" => {
                            Some(self.module.types.adt(h).args[1])
                        }
                        _ => None,
                    };
                    Some(self.expr(&args[0], err_hint)?)
                } else {
                    None
                };
                let err_ty = match (err_value, f) {
                    (Some(e), _) => self.reg_ty(e),
                    (_, Some(f)) => match self.tk(self.reg_ty(f)) {
                        TyKind::Fn(_, ret) => ret,
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                };
                let result_adt = self.adt_instance("Result", vec![payload_ty, err_ty])?;
                let result_ty = self.module.types.adt_ty(result_adt);
                let result = self.new_reg(result_ty);
                let (_, some, none) = self.split_on(r, 1);
                let join = self.new_block();
                self.switch_to(some);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: 1, index: 0 });
                self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 0, fields: vec![payload] });
                self.terminate(Term::Jump(join));
                self.switch_to(none);
                let e = match (err_value, f) {
                    (Some(e), _) => e,
                    (_, Some(f)) => self.call_closure_regs(f, Vec::new(), span)?,
                    _ => unreachable!(),
                };
                let e = self.coerce(e, err_ty, Some(span))?;
                self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 1, fields: vec![e] });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "map_err" if !is_option => {
                let f = self.function_arg(args, method, span)?;
                let TyKind::Fn(_, new_err) = self.tk(self.reg_ty(f)) else { unreachable!() };
                let result_adt = self.adt_instance("Result", vec![payload_ty, new_err])?;
                let result_ty = self.module.types.adt_ty(result_adt);
                let result = self.new_reg(result_ty);
                let (_, ok, fail) = self.split_on(r, 0);
                let join = self.new_block();
                self.switch_to(ok);
                let payload = self.new_reg(payload_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: 0, index: 0 });
                self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 0, fields: vec![payload] });
                self.terminate(Term::Jump(join));
                self.switch_to(fail);
                let fallback_args = self.failure_payload(r, &def, false);
                let e = self.call_closure_regs(f, fallback_args, span)?;
                let e = self.coerce(e, new_err, Some(span))?;
                self.emit(Instr::Variant { dst: result, adt: result_adt, variant: 1, fields: vec![e] });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            "ok" | "err" if !is_option => {
                self.arity(method, args, &[0], span)?;
                let (keep, keep_ty) = if method == "ok" { (0u32, payload_ty) } else { (1u32, def.variants()[1].fields[0].ty) };
                let option_adt = self.adt_instance("Option", vec![keep_ty])?;
                let option_ty = self.module.types.adt_ty(option_adt);
                let result = self.new_reg(option_ty);
                let (_, hit, miss) = self.split_on(r, keep as i64);
                let join = self.new_block();
                self.switch_to(hit);
                let payload = self.new_reg(keep_ty);
                self.emit(Instr::Payload { dst: payload, src: r, variant: keep, index: 0 });
                self.emit(Instr::Variant { dst: result, adt: option_adt, variant: 1, fields: vec![payload] });
                self.terminate(Term::Jump(join));
                self.switch_to(miss);
                self.emit(Instr::Variant { dst: result, adt: option_adt, variant: 0, fields: Vec::new() });
                self.terminate(Term::Jump(join));
                self.switch_to(join);
                Ok(result)
            }
            _ => self.err(Some(span), format!("{} has no method `{method}`", def.base)),
        }
    }

    fn vec2_method(&mut self, r: Reg, method: &str, args: &[Expr], span: Span) -> LResult<Reg> {
        let num = self.num_ty(NumKind::Num);
        let r_ty = self.reg_ty(r);
        let TyKind::Adt(adt) = self.tk(r_ty) else { unreachable!() };
        match method {
            "length" | "normalized" => {
                self.arity(method, args, &[0], span)?;
                let (f, ret) = if method == "length" { (RtFn::Vec2Length, num) } else { (RtFn::Vec2Normalized, r_ty) };
                Ok(self.rt(f, vec![r], ret))
            }
            "add" | "sub" | "scale" => {
                self.arity(method, args, &[1], span)?;
                let op = if method == "sub" { BinOp::Sub } else { BinOp::Add };
                let mut parts = Vec::new();
                if method == "scale" {
                    let k = self.expr_as(&args[0], num)?;
                    for i in 0..2u32 {
                        let c = self.new_reg(num);
                        self.emit(Instr::Get { dst: c, base: r, proj: Proj::Field(i) });
                        parts.push(self.arith(BinOp::Mul, NumKind::Num, num, c, k));
                    }
                } else {
                    let other = self.expr_as(&args[0], r_ty)?;
                    for i in 0..2u32 {
                        let (a, b) = (self.new_reg(num), self.new_reg(num));
                        self.emit(Instr::Get { dst: a, base: r, proj: Proj::Field(i) });
                        self.emit(Instr::Get { dst: b, base: other, proj: Proj::Field(i) });
                        parts.push(self.arith(op, NumKind::Num, num, a, b));
                    }
                }
                let dst = self.new_reg(r_ty);
                self.emit(Instr::Struct { dst, adt, fields: parts });
                Ok(dst)
            }
            _ => self.err(Some(span), format!("Vec2 has no method `{method}`")),
        }
    }
}
