//! Structural and type checks over a module. Lowering runs it on everything
//! it produces, so a lowering bug shows up as an "invalid IR" error instead of
//! a wrong answer later.

use super::ir::*;
use super::ty::*;

pub fn verify(module: &Module) -> Vec<String> {
    let mut problems = Vec::new();
    for (index, func) in module.funcs.iter().enumerate() {
        let mut v = Verifier { module, func, problems: Vec::new() };
        v.run();
        for p in v.problems {
            problems.push(format!("{} (#{index}): {p}", func.name));
        }
    }
    problems
}

struct Verifier<'m> {
    module: &'m Module,
    func: &'m Func,
    problems: Vec<String>,
}

impl<'m> Verifier<'m> {
    fn fail(&mut self, at: String, message: String) {
        self.problems.push(format!("{at}: {message}"));
    }

    fn reg_ok(&mut self, at: &str, reg: Reg) -> bool {
        if (reg.0 as usize) < self.func.regs.len() {
            true
        } else {
            self.fail(at.to_string(), format!("register r{} does not exist", reg.0));
            false
        }
    }

    fn ty(&self, reg: Reg) -> TyId {
        self.func.regs[reg.0 as usize]
    }

    fn show(&self, ty: TyId) -> String {
        self.module.types.show(ty)
    }

    fn kind(&self, ty: TyId) -> &'m TyKind {
        self.module.types.kind(ty)
    }

    fn run(&mut self) {
        let func = self.func;
        for p in &func.params {
            self.reg_ok("params", *p);
        }
        if func.blocks.is_empty() {
            self.fail("body".into(), "no blocks".into());
            return;
        }
        for (b, block) in func.blocks.iter().enumerate() {
            for (i, instr) in block.instrs.iter().enumerate() {
                let at = format!("b{b}[{i}]");
                if instr.uses().into_iter().chain(instr.dst()).all(|r| self.reg_ok(&at, r)) {
                    if let Err(message) = self.instr(instr) {
                        self.fail(at, message);
                    }
                }
            }
            let at = format!("b{b} terminator");
            if let Err(message) = self.term(&block.term) {
                self.fail(at, message);
            }
        }
    }

    fn expect(&self, what: &str, reg: Reg, want: TyId) -> Result<(), String> {
        if self.ty(reg) == want {
            Ok(())
        } else {
            Err(format!("{what}: expected {}, found {}", self.show(want), self.show(self.ty(reg))))
        }
    }

    fn is_num(&self, reg: Reg) -> Option<NumKind> {
        match self.kind(self.ty(reg)) {
            TyKind::Num(k) => Some(*k),
            _ => None,
        }
    }

    fn target(&self, b: BlockId) -> Result<(), String> {
        if (b.0 as usize) < self.func.blocks.len() {
            Ok(())
        } else {
            Err(format!("jump to missing block b{}", b.0))
        }
    }

    fn term(&self, term: &Term) -> Result<(), String> {
        match term {
            Term::Jump(b) => self.target(*b),
            Term::Branch { cond, then_, else_ } => {
                if cond.0 as usize >= self.func.regs.len() {
                    return Err("branch on a missing register".into());
                }
                if !matches!(self.kind(self.ty(*cond)), TyKind::Bool) {
                    return Err("branch condition is not a bool".into());
                }
                self.target(*then_)?;
                self.target(*else_)
            }
            Term::Switch { value, cases, default } => {
                if value.0 as usize >= self.func.regs.len() {
                    return Err("switch on a missing register".into());
                }
                match self.is_num(*value) {
                    Some(k) if k.is_int() => {}
                    _ => return Err("switch value is not an integer".into()),
                }
                for (_, b) in cases {
                    self.target(*b)?;
                }
                self.target(*default)
            }
            Term::Return(r) => {
                if r.0 as usize >= self.func.regs.len() {
                    return Err("return of a missing register".into());
                }
                self.expect("return value", *r, self.func.ret)
            }
            Term::Trap(_) => Ok(()),
        }
    }

    fn proj_ty(&self, base: Reg, proj: &Proj) -> Result<TyId, String> {
        let types = &self.module.types;
        match (self.kind(self.ty(base)), proj) {
            (TyKind::Adt(id), Proj::Field(i)) => match &types.adt(*id).body {
                AdtBody::Struct(fields) => fields
                    .get(*i as usize)
                    .map(|f| f.ty)
                    .ok_or_else(|| format!("field {i} out of range")),
                AdtBody::Enum(_) => Err("field of an enum".into()),
            },
            (TyKind::Tuple(items), Proj::Tuple(i)) => {
                items.get(*i as usize).copied().ok_or_else(|| format!("tuple element {i} out of range"))
            }
            (TyKind::List(item), Proj::Index(r)) => {
                if self.is_num(*r).is_none() {
                    return Err("list index is not a number".into());
                }
                Ok(*item)
            }
            (TyKind::Map(item), Proj::Key(r)) => {
                if !matches!(self.kind(self.ty(*r)), TyKind::Str) {
                    return Err("map key is not a string".into());
                }
                Ok(*item)
            }
            (other, p) => Err(format!("cannot project {p:?} out of {other:?}")),
        }
    }

    fn instr(&self, instr: &Instr) -> Result<(), String> {
        let types = &self.module.types;
        match instr {
            Instr::Const { dst, value } => {
                let ok = match (value, self.kind(self.ty(*dst))) {
                    (Const::Unit, TyKind::Unit) => true,
                    (Const::Bool(_), TyKind::Bool) => true,
                    (Const::Str(_), TyKind::Str) => true,
                    (Const::Int(_), TyKind::Num(k)) => k.is_int(),
                    (Const::Float(_), TyKind::Num(k)) => k.is_float(),
                    _ => false,
                };
                if ok { Ok(()) } else { Err(format!("constant {value:?} does not fit {}", self.show(self.ty(*dst)))) }
            }
            Instr::Mov { dst, src } => self.expect("mov", *src, self.ty(*dst)),
            Instr::Bin { dst, kind, a, b, .. } => {
                let ty = self.ty(*dst);
                if !matches!(self.kind(ty), TyKind::Num(k) if k == kind) {
                    return Err(format!("arithmetic result must be {}", kind.name()));
                }
                self.expect("left operand", *a, ty)?;
                self.expect("right operand", *b, ty)
            }
            Instr::Cmp { dst, op, a, b } => {
                if !matches!(self.kind(self.ty(*dst)), TyKind::Bool) {
                    return Err("comparison result is not a bool".into());
                }
                self.expect("right operand", *b, self.ty(*a))?;
                let ordered = !matches!(op, CmpOp::Eq | CmpOp::Ne);
                if ordered && !matches!(self.kind(self.ty(*a)), TyKind::Num(_) | TyKind::Str) {
                    return Err("ordering needs numbers or strings".into());
                }
                Ok(())
            }
            Instr::Not { dst, src } => {
                if !matches!(self.kind(self.ty(*src)), TyKind::Bool) {
                    return Err("`not` of a non-bool".into());
                }
                self.expect("not", *dst, self.ty(*src))
            }
            Instr::Neg { dst, src } => {
                if self.is_num(*src).is_none() {
                    return Err("negation of a non-number".into());
                }
                self.expect("neg", *dst, self.ty(*src))
            }
            Instr::Cast { dst, src } => {
                if self.is_num(*src).is_none() || self.is_num(*dst).is_none() {
                    return Err("cast between non-numbers".into());
                }
                Ok(())
            }
            Instr::ToStr { dst, .. } => {
                if matches!(self.kind(self.ty(*dst)), TyKind::Str) { Ok(()) } else { Err("to_str result is not a string".into()) }
            }
            Instr::Tuple { dst, items } => match self.kind(self.ty(*dst)) {
                TyKind::Tuple(tys) if tys.len() == items.len() => {
                    for (r, t) in items.iter().zip(tys) {
                        self.expect("tuple element", *r, *t)?;
                    }
                    Ok(())
                }
                other => Err(format!("tuple built as {other:?}")),
            },
            Instr::Struct { dst, adt, fields } => {
                self.expect("struct", *dst, self.adt_ty_of(*adt)?)?;
                let defs = types.adt(*adt).struct_fields();
                if defs.len() != fields.len() {
                    return Err(format!("struct has {} fields, given {}", defs.len(), fields.len()));
                }
                for (r, d) in fields.iter().zip(defs) {
                    self.expect(&format!("field {}", d.name), *r, d.ty)?;
                }
                Ok(())
            }
            Instr::Variant { dst, adt, variant, fields } => {
                self.expect("variant", *dst, self.adt_ty_of(*adt)?)?;
                let def = types.adt(*adt);
                let Some(v) = def.variants().get(*variant as usize) else {
                    return Err(format!("variant {variant} does not exist"));
                };
                if v.fields.len() != fields.len() {
                    return Err(format!("{} takes {} fields, given {}", v.name, v.fields.len(), fields.len()));
                }
                for (r, d) in fields.iter().zip(&v.fields) {
                    self.expect(&format!("field {}", d.name), *r, d.ty)?;
                }
                Ok(())
            }
            Instr::List { dst, items } => match self.kind(self.ty(*dst)) {
                TyKind::List(item) => {
                    for r in items {
                        self.expect("list element", *r, *item)?;
                    }
                    Ok(())
                }
                other => Err(format!("list built as {other:?}")),
            },
            Instr::Map { dst, entries } => match self.kind(self.ty(*dst)) {
                TyKind::Map(item) => {
                    for (_, r) in entries {
                        self.expect("map value", *r, *item)?;
                    }
                    Ok(())
                }
                other => Err(format!("map built as {other:?}")),
            },
            Instr::Get { dst, base, proj } | Instr::Take { dst, base, proj } => {
                let ty = self.proj_ty(*base, proj)?;
                self.expect("projection", *dst, ty)
            }
            Instr::Set { base, proj, src } => {
                let ty = self.proj_ty(*base, proj)?;
                self.expect("stored value", *src, ty)
            }
            Instr::Tag { dst, src } => match self.kind(self.ty(*src)) {
                TyKind::Adt(id) if types.adt(*id).is_enum() => {
                    if matches!(self.kind(self.ty(*dst)), TyKind::Num(NumKind::I64)) { Ok(()) } else { Err("tag is not an i64".into()) }
                }
                other => Err(format!("tag of {other:?}")),
            },
            Instr::Payload { dst, src, variant, index } => match self.kind(self.ty(*src)) {
                TyKind::Adt(id) => {
                    let v = types
                        .adt(*id)
                        .variants()
                        .get(*variant as usize)
                        .ok_or_else(|| format!("variant {variant} does not exist"))?;
                    let f = v.fields.get(*index as usize).ok_or_else(|| format!("payload {index} does not exist"))?;
                    self.expect("payload", *dst, f.ty)
                }
                other => Err(format!("payload of {other:?}")),
            },
            Instr::Rt { dst, f, args } => self.rt(*dst, *f, args),
            Instr::Host { dst, f, .. } => {
                let want = match f {
                    HostFn::Print | HostFn::Dbg | HostFn::Error => matches!(self.kind(self.ty(*dst)), TyKind::Unit),
                    HostFn::ReadLine | HostFn::ReadKey => matches!(self.kind(self.ty(*dst)), TyKind::Str),
                };
                if want { Ok(()) } else { Err("host call result has the wrong type".into()) }
            }
            Instr::Call { dst, func, args } => {
                let callee = self.module.funcs.get(func.0 as usize).ok_or("call of a missing function")?;
                if callee.params.len() != args.len() {
                    return Err(format!("`{}` takes {} arguments, given {}", callee.name, callee.params.len(), args.len()));
                }
                for (r, p) in args.iter().zip(&callee.params) {
                    self.expect(&format!("argument of `{}`", callee.name), *r, callee.reg_ty(*p))?;
                }
                self.expect("call result", *dst, callee.ret)
            }
            Instr::CallClosure { dst, callee, args } => match self.kind(self.ty(*callee)) {
                TyKind::Fn(params, ret) => {
                    if params.len() != args.len() {
                        return Err(format!("closure takes {} arguments, given {}", params.len(), args.len()));
                    }
                    for (r, p) in args.iter().zip(params) {
                        self.expect("closure argument", *r, *p)?;
                    }
                    self.expect("closure result", *dst, *ret)
                }
                other => Err(format!("call of {other:?}")),
            },
            Instr::Closure { dst, func, captures } => {
                let callee = self.module.funcs.get(func.0 as usize).ok_or("closure of a missing function")?;
                let n = captures.len();
                if n != callee.ncaptures as usize {
                    return Err(format!("`{}` captures {} values, given {n}", callee.name, callee.ncaptures));
                }
                for (r, p) in captures.iter().zip(&callee.params) {
                    self.expect("captured value", *r, callee.reg_ty(*p))?;
                }
                match self.kind(self.ty(*dst)) {
                    TyKind::Fn(params, ret) => {
                        let rest: Vec<TyId> = callee.params[n..].iter().map(|p| callee.reg_ty(*p)).collect();
                        if *params != rest || *ret != callee.ret {
                            return Err(format!("closure type {} does not match `{}`", self.show(self.ty(*dst)), callee.name));
                        }
                        Ok(())
                    }
                    other => Err(format!("closure typed {other:?}")),
                }
            }
            Instr::GlobalGet { dst, global } => {
                let g = self.module.globals.get(global.0 as usize).ok_or("missing global")?;
                self.expect("global", *dst, g.ty)
            }
            Instr::GlobalSet { global, src } => {
                let g = self.module.globals.get(global.0 as usize).ok_or("missing global")?;
                self.expect("global", *src, g.ty)
            }
        }
    }

    fn adt_ty_of(&self, adt: AdtId) -> Result<TyId, String> {
        // The interner is append only; find the type that names this ADT.
        (0..self.module.types.len() as u32)
            .map(TyId)
            .find(|t| matches!(self.kind(*t), TyKind::Adt(a) if *a == adt))
            .ok_or_else(|| "ADT without a type".to_string())
    }

    fn rt(&self, dst: Reg, f: RtFn, args: &[Reg]) -> Result<(), String> {
        let types = &self.module.types;
        let name = f.name();
        let count = |allowed: &[usize]| -> Result<(), String> {
            if allowed.contains(&args.len()) { Ok(()) } else { Err(format!("{name} takes {allowed:?} arguments, given {}", args.len())) }
        };
        let is = |reg: Reg, pred: &dyn Fn(&TyKind) -> bool| pred(self.kind(self.ty(reg)));
        let strs = |r: Reg| is(r, &|k| matches!(k, TyKind::Str));
        let num = |r: Reg| is(r, &|k| matches!(k, TyKind::Num(NumKind::Num)));
        let list_of = |r: Reg| -> Option<TyId> {
            match self.kind(self.ty(r)) { TyKind::List(t) => Some(*t), _ => None }
        };
        let map_of = |r: Reg| -> Option<TyId> {
            match self.kind(self.ty(r)) { TyKind::Map(t) => Some(*t), _ => None }
        };
        let ret = |pred: &dyn Fn(&TyKind) -> bool, what: &str| -> Result<(), String> {
            if pred(self.kind(self.ty(dst))) { Ok(()) } else { Err(format!("{name} must produce {what}")) }
        };
        let need = |ok: bool, what: &str| -> Result<(), String> {
            if ok { Ok(()) } else { Err(format!("{name}: {what}")) }
        };
        // `Option<t>` / `Result<..>` results of a given payload type.
        let option_of = |t: TyId| -> bool {
            match self.kind(self.ty(dst)) {
                TyKind::Adt(a) => { let d = types.adt(*a); d.base == "Option" && d.args == [t] }
                _ => false,
            }
        };
        match f {
            RtFn::StrConcat => {
                need(args.iter().all(|a| strs(*a)), "arguments must be strings")?;
                ret(&|k| matches!(k, TyKind::Str), "a string")
            }
            RtFn::StrLen => { count(&[1])?; need(strs(args[0]), "string expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::StrIsEmpty => { count(&[1])?; need(strs(args[0]), "string expected")?; ret(&|k| matches!(k, TyKind::Bool), "bool") }
            RtFn::StrTrim | RtFn::StrUpper | RtFn::StrLower => { count(&[1])?; need(strs(args[0]), "string expected")?; ret(&|k| matches!(k, TyKind::Str), "a string") }
            RtFn::StrContains | RtFn::StrStartsWith | RtFn::StrEndsWith => { count(&[2])?; need(strs(args[0]) && strs(args[1]), "strings expected")?; ret(&|k| matches!(k, TyKind::Bool), "bool") }
            RtFn::StrSlice => { count(&[2, 3])?; need(strs(args[0]) && args[1..].iter().all(|a| num(*a)), "string and numbers expected")?; ret(&|k| matches!(k, TyKind::Str), "a string") }
            RtFn::StrSplit => {
                count(&[2])?; need(strs(args[0]) && strs(args[1]), "strings expected")?;
                ret(&|k| matches!(k, TyKind::List(t) if matches!(types.kind(*t), TyKind::Str)), "[string]")
            }
            RtFn::StrReplace => { count(&[3])?; need(args.iter().all(|a| strs(*a)), "strings expected")?; ret(&|k| matches!(k, TyKind::Str), "a string") }
            RtFn::ListLen => { count(&[1])?; need(list_of(args[0]).is_some(), "list expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::ListIsEmpty => { count(&[1])?; need(list_of(args[0]).is_some(), "list expected")?; ret(&|k| matches!(k, TyKind::Bool), "bool") }
            RtFn::ListPush => {
                count(&[2])?;
                let t = list_of(args[0]).ok_or("list expected")?;
                need(self.ty(args[1]) == t, "element type differs")?;
                ret(&|k| matches!(k, TyKind::Unit), "unit")
            }
            RtFn::ListPop => { count(&[1])?; let t = list_of(args[0]).ok_or("list expected")?; need(option_of(t), "result must be Option of the element")?; Ok(()) }
            RtFn::ListRemove => {
                count(&[2])?;
                let t = list_of(args[0]).ok_or("list expected")?;
                need(num(args[1]), "index must be a number")?;
                need(self.ty(dst) == t, "result must be the element")
            }
            RtFn::ListReverse | RtFn::ListSort => { count(&[1])?; need(list_of(args[0]).is_some(), "list expected")?; need(self.ty(dst) == self.ty(args[0]), "result must be the same list type") }
            RtFn::ListSlice => { count(&[2, 3])?; need(list_of(args[0]).is_some() && args[1..].iter().all(|a| num(*a)), "list and numbers expected")?; need(self.ty(dst) == self.ty(args[0]), "result must be the same list type") }
            RtFn::ListContains => {
                count(&[2])?;
                let t = list_of(args[0]).ok_or("list expected")?;
                need(self.ty(args[1]) == t, "element type differs")?;
                ret(&|k| matches!(k, TyKind::Bool), "bool")
            }
            RtFn::ListJoin => { count(&[2])?; need(list_of(args[0]).is_some() && strs(args[1]), "list and string expected")?; ret(&|k| matches!(k, TyKind::Str), "a string") }
            RtFn::MapLen => { count(&[1])?; need(map_of(args[0]).is_some(), "map expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::MapIsEmpty => { count(&[1])?; need(map_of(args[0]).is_some(), "map expected")?; ret(&|k| matches!(k, TyKind::Bool), "bool") }
            RtFn::MapHas => { count(&[2])?; need(map_of(args[0]).is_some() && strs(args[1]), "map and string expected")?; ret(&|k| matches!(k, TyKind::Bool), "bool") }
            RtFn::MapGet | RtFn::MapRemove => {
                count(&[2])?;
                let t = map_of(args[0]).ok_or("map expected")?;
                need(strs(args[1]), "key must be a string")?;
                need(option_of(t), "result must be Option of the value")
            }
            RtFn::MapSet => {
                count(&[3])?;
                let t = map_of(args[0]).ok_or("map expected")?;
                need(strs(args[1]) && self.ty(args[2]) == t, "key string and value expected")?;
                ret(&|k| matches!(k, TyKind::Unit), "unit")
            }
            RtFn::MapKeys => { count(&[1])?; need(map_of(args[0]).is_some(), "map expected")?; ret(&|k| matches!(k, TyKind::List(t) if matches!(types.kind(*t), TyKind::Str)), "[string]") }
            RtFn::MapValues => {
                count(&[1])?;
                let t = map_of(args[0]).ok_or("map expected")?;
                need(matches!(self.kind(self.ty(dst)), TyKind::List(e) if *e == t), "result must list the values")
            }
            RtFn::Sqrt | RtFn::Abs | RtFn::Sign => { count(&[1])?; need(num(args[0]), "number expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::Min | RtFn::Max => { count(&[2])?; need(args.iter().all(|a| num(*a)), "numbers expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::Clamp => { count(&[3])?; need(args.iter().all(|a| num(*a)), "numbers expected")?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::ParseNumber => {
                count(&[1])?; need(strs(args[0]), "string expected")?;
                ret(&|k| matches!(k, TyKind::Adt(a) if types.adt(*a).base == "Result"), "a Result")
            }
            RtFn::Vec2Length => { count(&[1])?; ret(&|k| matches!(k, TyKind::Num(NumKind::Num)), "number") }
            RtFn::Vec2Normalized => { count(&[1])?; need(self.ty(dst) == self.ty(args[0]), "result must be a Vec2") }
        }
    }
}
