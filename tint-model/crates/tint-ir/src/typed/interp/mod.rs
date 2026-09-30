//! Reference interpreter of the typed IR.
//!
//! It defines what every instruction means. Faster engines (a register VM, a
//! native backend) must agree with it; the conformance suite runs on both.
//! Values are reference counted and copy-on-write, so `Mov` is cheap and a
//! write never shows through another register.

mod fmt;
mod rt;

pub use fmt::{debug, display, render};

use super::ir::*;
use super::ty::*;
use super::ui::{UiEvent, UiValue};
use std::collections::BTreeMap;
use std::rc::Rc;

/// A run-time error: the program stops with a message.
#[derive(Debug, Clone, PartialEq)]
pub struct Trap {
    pub msg: String,
}

impl Trap {
    pub fn new(msg: impl Into<String>) -> Trap {
        Trap { msg: msg.into() }
    }
}

impl std::fmt::Display for Trap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

type Res<T> = Result<T, Trap>;

#[derive(Debug, Clone)]
pub enum Val {
    /// A register nothing was written to yet.
    Undef,
    Unit,
    Bool(bool),
    /// Every integer kind; `u64` as its bit pattern.
    Int(i64),
    /// Every float kind; `f32` values are stored rounded.
    Float(f64),
    Str(Rc<str>),
    List(Rc<Vec<Val>>),
    Map(Rc<BTreeMap<String, Val>>),
    Tuple(Rc<Vec<Val>>),
    Adt(Rc<AdtVal>),
    Closure(Rc<ClosureVal>),
}

#[derive(Debug, Clone)]
pub struct AdtVal {
    pub adt: AdtId,
    /// Variant index; 0 for structs.
    pub tag: u32,
    pub fields: Vec<Val>,
}

#[derive(Debug, Clone)]
pub struct ClosureVal {
    pub func: FuncId,
    pub captures: Vec<Val>,
}

impl Val {
    pub fn str(s: impl AsRef<str>) -> Val {
        Val::Str(Rc::from(s.as_ref()))
    }

    pub fn list(items: Vec<Val>) -> Val {
        Val::List(Rc::new(items))
    }
}

pub struct Interp<'m> {
    pub module: &'m Module,
    globals: Vec<Val>,
    /// Lines written by `print`, `dbg` and `error`.
    pub output: Vec<String>,
    /// Also write output lines to stdout.
    pub echo: bool,
    /// The last `output` entry came from `print` and has no newline yet.
    open_line: bool,
    depth: usize,
    pub max_depth: usize,
    /// Remaining instruction budget; `None` is unlimited.
    pub steps_left: Option<u64>,
    /// What `Ui` functions emitted, in order.
    pub ui_events: Vec<UiEvent>,
}

impl<'m> Interp<'m> {
    pub fn new(module: &'m Module) -> Self {
        Interp {
            module,
            globals: vec![Val::Undef; module.globals.len()],
            output: Vec::new(),
            echo: false,
            open_line: false,
            depth: 0,
            max_depth: 1500,
            steps_left: None,
            ui_events: Vec::new(),
        }
    }

    /// Runs one runtime-library call on already-built values. A native backend
    /// uses this for the calls it does not implement itself, so that they
    /// mean exactly what they mean here. Returns the result and the argument
    /// values afterwards (the first one changes for `RtFn::mutates_first`).
    pub fn rt_values(
        &mut self,
        f: RtFn,
        arg_tys: &[TyId],
        dst_ty: TyId,
        args: Vec<Val>,
    ) -> Res<(Val, Vec<Val>)> {
        let n = args.len();
        let mut tys = arg_tys.to_vec();
        tys.push(dst_ty);
        let func = Func {
            name: "<rt>".into(),
            kind: FuncKind::Fn,
            params: Vec::new(),
            ncaptures: 0,
            ret: dst_ty,
            regs: tys,
            blocks: Vec::new(),
        };
        let mut regs = args;
        regs.push(Val::Undef);
        let arg_regs: Vec<Reg> = (0..n).map(|i| Reg(i as u32)).collect();
        let dst = Reg(n as u32);
        self.call_rt(&func, &mut regs, dst, f, &arg_regs)?;
        let result = regs[n].clone();
        regs.truncate(n);
        Ok((result, regs))
    }

    /// `a <op> b` for two values of type `ty`, as the `Cmp` instruction.
    pub fn compare_values(&self, ty: TyId, op: CmpOp, a: &Val, b: &Val) -> Res<bool> {
        compare(&self.module.types, ty, op, a, b)
    }

    /// Runs the global initializers.
    pub fn init(&mut self) -> Res<()> {
        if let Some(init) = self.module.init {
            self.call(init, Vec::new())?;
        }
        Ok(())
    }

    /// Initializes globals and calls the function `name`.
    pub fn run(&mut self, name: &str, args: Vec<Val>) -> Res<(Val, TyId)> {
        let id = *self
            .module
            .functions
            .get(name)
            .ok_or_else(|| Trap::new(format!("unknown function `{name}`")))?;
        self.init()?;
        let ret = self.module.func(id).ret;
        Ok((self.call(id, args)?, ret))
    }

    /// Overwrites the global `name` (a `state` variable, `theme`, ...).
    pub fn set_global(&mut self, name: &str, value: Val) -> Res<()> {
        let index = self
            .module
            .globals
            .iter()
            .position(|g| g.name == name)
            .ok_or_else(|| Trap::new(format!("unknown global `{name}`")))?;
        self.globals[index] = value;
        Ok(())
    }

    pub fn global(&self, name: &str) -> Option<&Val> {
        let index = self.module.globals.iter().position(|g| g.name == name)?;
        self.globals.get(index)
    }

    /// Calls the `ui fn` `name` (globals must be initialized: see `init`) and
    /// returns what it emitted; feed that to `ui::replay`.
    pub fn run_ui(&mut self, name: &str, args: Vec<Val>) -> Res<Vec<UiEvent>> {
        let id = *self
            .module
            .functions
            .get(name)
            .ok_or_else(|| Trap::new(format!("unknown function `{name}`")))?;
        if self.module.func(id).kind != FuncKind::Ui {
            return Err(Trap::new(format!("`{name}` is not a ui fn")));
        }
        self.ui_events.clear();
        self.call(id, args)?;
        Ok(std::mem::take(&mut self.ui_events))
    }

    pub fn call(&mut self, id: FuncId, args: Vec<Val>) -> Res<Val> {
        let func = self.module.func(id);
        if args.len() != func.params.len() {
            return Err(Trap::new(format!(
                "`{}` takes {} arguments, got {}",
                func.name,
                func.params.len(),
                args.len()
            )));
        }
        if self.depth >= self.max_depth {
            return Err(Trap::new("stack overflow: call depth limit exceeded"));
        }
        self.depth += 1;
        let result = self.exec(func, args);
        self.depth -= 1;
        result
    }

    fn exec(&mut self, func: &'m Func, args: Vec<Val>) -> Res<Val> {
        let mut regs = vec![Val::Undef; func.regs.len()];
        for (param, arg) in func.params.iter().zip(args) {
            regs[param.0 as usize] = arg;
        }
        let mut block = 0usize;
        loop {
            let b = &func.blocks[block];
            for instr in &b.instrs {
                self.tick()?;
                self.step(func, &mut regs, instr)?;
            }
            self.tick()?;
            match &b.term {
                Term::Jump(next) => block = next.0 as usize,
                Term::Branch { cond, then_, else_ } => {
                    let taken = match &regs[cond.0 as usize] {
                        Val::Bool(b) => *b,
                        other => return Err(bad("branch on a non-bool", other)),
                    };
                    block = if taken { then_.0 } else { else_.0 } as usize;
                }
                Term::Switch {
                    value,
                    cases,
                    default,
                } => {
                    let v = match &regs[value.0 as usize] {
                        Val::Int(i) => *i,
                        other => return Err(bad("switch on a non-integer", other)),
                    };
                    block = cases
                        .iter()
                        .find(|(case, _)| *case == v)
                        .map(|(_, b)| b.0)
                        .unwrap_or(default.0) as usize;
                }
                Term::Return(r) => {
                    return Ok(std::mem::replace(&mut regs[r.0 as usize], Val::Undef))
                }
                Term::Trap(msg) => return Err(Trap::new(msg.clone())),
            }
        }
    }

    fn tick(&mut self) -> Res<()> {
        if let Some(left) = &mut self.steps_left {
            if *left == 0 {
                return Err(Trap::new("step limit exceeded"));
            }
            *left -= 1;
        }
        Ok(())
    }

    fn ty_of(&self, func: &Func, reg: Reg) -> &'m TyKind {
        self.module.types.kind(func.reg_ty(reg))
    }

    fn num_kind(&self, func: &Func, reg: Reg) -> Res<NumKind> {
        match self.ty_of(func, reg) {
            TyKind::Num(kind) => Ok(*kind),
            other => Err(Trap::new(format!(
                "expected a number register, found {other:?}"
            ))),
        }
    }

    fn step(&mut self, func: &'m Func, regs: &mut Vec<Val>, instr: &'m Instr) -> Res<()> {
        let types = &self.module.types;
        match instr {
            Instr::Const { dst, value } => {
                regs[dst.0 as usize] = match value {
                    Const::Unit => Val::Unit,
                    Const::Bool(b) => Val::Bool(*b),
                    Const::Int(i) => Val::Int(*i),
                    Const::Float(f) => Val::Float(*f),
                    Const::Str(s) => Val::str(s),
                };
            }
            Instr::Mov { dst, src } => {
                let v = regs[src.0 as usize].clone();
                regs[dst.0 as usize] = v;
            }
            Instr::Bin {
                dst,
                op,
                kind,
                a,
                b,
            } => {
                let r = arith(*op, *kind, &regs[a.0 as usize], &regs[b.0 as usize])?;
                regs[dst.0 as usize] = r;
            }
            Instr::Cmp { dst, op, a, b } => {
                let ty = func.reg_ty(*a);
                let r = compare(types, ty, *op, &regs[a.0 as usize], &regs[b.0 as usize])?;
                regs[dst.0 as usize] = Val::Bool(r);
            }
            Instr::Not { dst, src } => {
                let v = match &regs[src.0 as usize] {
                    Val::Bool(b) => !*b,
                    other => return Err(bad("`!` on a non-bool", other)),
                };
                regs[dst.0 as usize] = Val::Bool(v);
            }
            Instr::Neg { dst, src } => {
                let kind = self.num_kind(func, *src)?;
                let r = match (&regs[src.0 as usize], kind.is_float()) {
                    (Val::Float(f), true) => Val::Float(-*f),
                    (Val::Int(_), false) => {
                        arith(BinOp::Sub, kind, &Val::Int(0), &regs[src.0 as usize])?
                    }
                    (other, _) => return Err(bad("negation of a non-number", other)),
                };
                regs[dst.0 as usize] = r;
            }
            Instr::Cast { dst, src } => {
                let from = self.num_kind(func, *src)?;
                let to = self.num_kind(func, *dst)?;
                let r = cast_num(from, to, &regs[src.0 as usize])?;
                regs[dst.0 as usize] = r;
            }
            Instr::ToStr { dst, src } => {
                let text = display(self.module, func.reg_ty(*src), &regs[src.0 as usize]);
                regs[dst.0 as usize] = Val::str(text);
            }
            Instr::Tuple { dst, items } => {
                let items = items.iter().map(|r| regs[r.0 as usize].clone()).collect();
                regs[dst.0 as usize] = Val::Tuple(Rc::new(items));
            }
            Instr::Struct { dst, adt, fields } => {
                let fields = fields.iter().map(|r| regs[r.0 as usize].clone()).collect();
                regs[dst.0 as usize] = Val::Adt(Rc::new(AdtVal {
                    adt: *adt,
                    tag: 0,
                    fields,
                }));
            }
            Instr::Variant {
                dst,
                adt,
                variant,
                fields,
            } => {
                let fields = fields.iter().map(|r| regs[r.0 as usize].clone()).collect();
                regs[dst.0 as usize] = Val::Adt(Rc::new(AdtVal {
                    adt: *adt,
                    tag: *variant,
                    fields,
                }));
            }
            Instr::List { dst, items } => {
                let items = items.iter().map(|r| regs[r.0 as usize].clone()).collect();
                regs[dst.0 as usize] = Val::List(Rc::new(items));
            }
            Instr::Map { dst, entries } => {
                let mut map = BTreeMap::new();
                for (key, reg) in entries {
                    map.insert(key.clone(), regs[reg.0 as usize].clone());
                }
                regs[dst.0 as usize] = Val::Map(Rc::new(map));
            }
            Instr::Get { dst, base, proj } => {
                let v = get_proj(&regs[base.0 as usize], proj, regs)?;
                regs[dst.0 as usize] = v;
            }
            Instr::Take { dst, base, proj } => {
                let key = resolve_proj(proj, regs)?;
                let v = take_proj(&mut regs[base.0 as usize], key)?;
                regs[dst.0 as usize] = v;
            }
            Instr::Set { base, proj, src } => {
                let key = resolve_proj(proj, regs)?;
                let value = regs[src.0 as usize].clone();
                set_proj(&mut regs[base.0 as usize], key, value)?;
            }
            Instr::Tag { dst, src } => {
                let tag = match &regs[src.0 as usize] {
                    Val::Adt(a) => a.tag as i64,
                    other => return Err(bad("tag of a non-enum", other)),
                };
                regs[dst.0 as usize] = Val::Int(tag);
            }
            Instr::Payload {
                dst,
                src,
                variant,
                index,
            } => {
                let v = match &regs[src.0 as usize] {
                    Val::Adt(a) if a.tag == *variant => a.fields[*index as usize].clone(),
                    other => return Err(bad("payload of the wrong variant", other)),
                };
                regs[dst.0 as usize] = v;
            }
            Instr::Rt { dst, f, args } => self.call_rt(func, regs, *dst, *f, args)?,
            Instr::Host { dst, f, args } => {
                let v = self.call_host(func, regs, *f, args);
                regs[dst.0 as usize] = v;
            }
            Instr::Call {
                dst,
                func: callee,
                args,
            } => {
                let args = args.iter().map(|r| regs[r.0 as usize].clone()).collect();
                let v = self.call(*callee, args)?;
                regs[dst.0 as usize] = v;
            }
            Instr::CallClosure { dst, callee, args } => {
                let closure = match &regs[callee.0 as usize] {
                    Val::Closure(c) => c.clone(),
                    other => return Err(bad("call of a non-function", other)),
                };
                let mut all = closure.captures.clone();
                all.extend(args.iter().map(|r| regs[r.0 as usize].clone()));
                let v = self.call(closure.func, all)?;
                regs[dst.0 as usize] = v;
            }
            Instr::Closure {
                dst,
                func: callee,
                captures,
            } => {
                let captures = captures
                    .iter()
                    .map(|r| regs[r.0 as usize].clone())
                    .collect();
                regs[dst.0 as usize] = Val::Closure(Rc::new(ClosureVal {
                    func: *callee,
                    captures,
                }));
            }
            Instr::GlobalGet { dst, global } => {
                regs[dst.0 as usize] = self.globals[global.0 as usize].clone();
            }
            Instr::UiOpen { template, values } => {
                let vals = values
                    .iter()
                    .map(|r| match &regs[r.0 as usize] {
                        Val::Int(i) => Ok(UiValue::Number(*i as f64)),
                        Val::Float(f) => Ok(UiValue::Number(*f)),
                        Val::Str(s) => Ok(UiValue::Str(s.to_string())),
                        Val::Bool(b) => Ok(UiValue::Bool(*b)),
                        other => Err(bad("non-scalar UI value", other)),
                    })
                    .collect::<Res<Vec<_>>>()?;
                self.ui_events.push(UiEvent::Open {
                    template: *template,
                    values: vals,
                });
            }
            Instr::UiClose => self.ui_events.push(UiEvent::Close),
            Instr::UiText { src } => {
                let text = display(self.module, func.reg_ty(*src), &regs[src.0 as usize]);
                self.ui_events.push(UiEvent::Text(text));
            }
            Instr::UiTokens { template } => {
                self.ui_events.push(UiEvent::Tokens {
                    template: *template,
                });
            }
            Instr::GlobalSet { global, src } => {
                self.globals[global.0 as usize] = regs[src.0 as usize].clone();
            }
        }
        Ok(())
    }

    fn push_line(&mut self, line: String) {
        if self.echo {
            println!("{line}");
        }
        if std::mem::take(&mut self.open_line) {
            self.output.last_mut().expect("open line").push_str(&line);
        } else {
            self.output.push(line);
        }
    }

    /// `print`: text without a trailing newline.
    fn push_text(&mut self, text: String) {
        if self.echo {
            use std::io::Write;
            print!("{text}");
            let _ = std::io::stdout().flush();
        }
        if self.open_line {
            self.output.last_mut().expect("open line").push_str(&text);
        } else {
            self.output.push(text);
            self.open_line = true;
        }
    }

    fn call_host(&mut self, func: &Func, regs: &[Val], f: HostFn, args: &[Reg]) -> Val {
        match f {
            HostFn::Println => {
                for arg in args {
                    let line = display(self.module, func.reg_ty(*arg), &regs[arg.0 as usize]);
                    self.push_line(line);
                }
                Val::Unit
            }
            HostFn::Print => {
                for arg in args {
                    let text = display(self.module, func.reg_ty(*arg), &regs[arg.0 as usize]);
                    self.push_text(text);
                }
                Val::Unit
            }
            HostFn::Dbg => {
                let parts: Vec<String> = args
                    .iter()
                    .map(|a| debug(self.module, func.reg_ty(*a), &regs[a.0 as usize]))
                    .collect();
                self.push_line(format!("DBG: [{}]", parts.join(", ")));
                Val::Unit
            }
            HostFn::Error => {
                for arg in args {
                    let line = debug(self.module, func.reg_ty(*arg), &regs[arg.0 as usize]);
                    self.push_line(format!("ERROR: {line}"));
                }
                Val::Unit
            }
            HostFn::ReadLine | HostFn::ReadKey => Val::str(""),
        }
    }
}

pub(crate) fn bad(what: &str, found: &Val) -> Trap {
    Trap::new(format!("internal error: {what}: {found:?}"))
}

// ---------------------------------------------------------------- numbers

pub(crate) fn int_of(kind: NumKind, v: &Val) -> Res<i128> {
    match v {
        Val::Int(i) => Ok(if kind == NumKind::U64 {
            (*i as u64) as i128
        } else {
            *i as i128
        }),
        other => Err(bad("expected an integer", other)),
    }
}

pub(crate) fn int_to(kind: NumKind, value: i128) -> Res<Val> {
    let (lo, hi) = kind.int_range();
    if value < lo || value > hi {
        return Err(Trap::new(format!(
            "{} arithmetic overflow: {value}",
            kind.name()
        )));
    }
    Ok(Val::Int(if kind == NumKind::U64 {
        (value as u64) as i64
    } else {
        value as i64
    }))
}

fn arith(op: BinOp, kind: NumKind, a: &Val, b: &Val) -> Res<Val> {
    if kind.is_float() {
        let (x, y) = match (a, b) {
            (Val::Float(x), Val::Float(y)) => (*x, *y),
            _ => return Err(bad("float arithmetic on non-floats", a)),
        };
        let r = if kind == NumKind::F32 {
            let (x, y) = (x as f32, y as f32);
            (match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                BinOp::Rem => x % y,
            }) as f64
        } else {
            match op {
                BinOp::Add => x + y,
                BinOp::Sub => x - y,
                BinOp::Mul => x * y,
                BinOp::Div => x / y,
                BinOp::Rem => x % y,
            }
        };
        return Ok(Val::Float(r));
    }
    let (x, y) = (int_of(kind, a)?, int_of(kind, b)?);
    let r = match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div | BinOp::Rem => {
            if y == 0 {
                return Err(Trap::new("division by zero"));
            }
            if op == BinOp::Div {
                x / y
            } else {
                x % y
            }
        }
    };
    int_to(kind, r)
}

fn compare(types: &TypeTable, ty: TyId, op: CmpOp, a: &Val, b: &Val) -> Res<bool> {
    match op {
        CmpOp::Eq => return val_eq(types, ty, a, b),
        CmpOp::Ne => return val_eq(types, ty, a, b).map(|eq| !eq),
        _ => {}
    }
    use std::cmp::Ordering;
    let ord: Option<Ordering> = match types.kind(ty) {
        TyKind::Num(kind) if kind.is_float() => match (a, b) {
            (Val::Float(x), Val::Float(y)) => x.partial_cmp(y),
            _ => return Err(bad("comparison of non-floats", a)),
        },
        TyKind::Num(kind) => Some(int_of(*kind, a)?.cmp(&int_of(*kind, b)?)),
        TyKind::Str => match (a, b) {
            (Val::Str(x), Val::Str(y)) => Some(x.as_ref().cmp(y.as_ref())),
            _ => return Err(bad("comparison of non-strings", a)),
        },
        other => return Err(Trap::new(format!("cannot order values of type {other:?}"))),
    };
    Ok(match (op, ord) {
        (_, None) => false,
        (CmpOp::Lt, Some(o)) => o == Ordering::Less,
        (CmpOp::Le, Some(o)) => o != Ordering::Greater,
        (CmpOp::Gt, Some(o)) => o == Ordering::Greater,
        (CmpOp::Ge, Some(o)) => o != Ordering::Less,
        _ => unreachable!(),
    })
}

/// Structural equality of two values of type `ty`.
pub(crate) fn val_eq(types: &TypeTable, ty: TyId, a: &Val, b: &Val) -> Res<bool> {
    Ok(match (types.kind(ty), a, b) {
        (_, Val::Unit, Val::Unit) => true,
        (_, Val::Bool(x), Val::Bool(y)) => x == y,
        (_, Val::Int(x), Val::Int(y)) => x == y,
        (_, Val::Float(x), Val::Float(y)) => x == y,
        (_, Val::Str(x), Val::Str(y)) => x == y,
        (TyKind::List(item), Val::List(x), Val::List(y)) => {
            x.len() == y.len() && all_eq(types, std::iter::repeat(*item), x, y)?
        }
        (TyKind::Tuple(items), Val::Tuple(x), Val::Tuple(y)) => {
            all_eq(types, items.iter().copied(), x, y)?
        }
        (TyKind::Map(item), Val::Map(x), Val::Map(y)) => {
            if x.len() != y.len() {
                false
            } else {
                let mut same = true;
                for (key, value) in x.iter() {
                    match y.get(key) {
                        Some(other) if val_eq(types, *item, value, other)? => {}
                        _ => {
                            same = false;
                            break;
                        }
                    }
                }
                same
            }
        }
        (TyKind::Adt(_), Val::Adt(x), Val::Adt(y)) => {
            if x.adt != y.adt || x.tag != y.tag {
                false
            } else {
                let def = types.adt(x.adt);
                let field_tys: Vec<TyId> = match &def.body {
                    AdtBody::Struct(fields) => fields.iter().map(|f| f.ty).collect(),
                    AdtBody::Enum(variants) => variants[x.tag as usize]
                        .fields
                        .iter()
                        .map(|f| f.ty)
                        .collect(),
                };
                all_eq(types, field_tys.into_iter(), &x.fields, &y.fields)?
            }
        }
        (TyKind::Fn(..), _, _) => return Err(Trap::new("functions cannot be compared")),
        _ => false,
    })
}

fn all_eq(types: &TypeTable, tys: impl Iterator<Item = TyId>, a: &[Val], b: &[Val]) -> Res<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for ((ty, x), y) in tys.zip(a).zip(b) {
        if !val_eq(types, ty, x, y)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `as`: converts between numeric kinds, failing when the value does not fit
/// or a float is not integral.
pub fn cast_num(from: NumKind, to: NumKind, v: &Val) -> Res<Val> {
    if to.is_float() {
        let x = match v {
            Val::Int(i) => {
                if from == NumKind::U64 {
                    (*i as u64) as f64
                } else {
                    *i as f64
                }
            }
            Val::Float(f) => *f,
            other => return Err(bad("cast of a non-number", other)),
        };
        if to == NumKind::F32 {
            if !x.is_finite() || x > f32::MAX as f64 || x < -(f32::MAX as f64) {
                return Err(Trap::new(format!(
                    "numeric cast failed: cannot convert {x} to f32: value is out of range"
                )));
            }
            return Ok(Val::Float((x as f32) as f64));
        }
        return Ok(Val::Float(x));
    }
    let (lo, hi) = to.int_range();
    let wide: i128 = match v {
        Val::Float(f) => {
            if !f.is_finite() || f.fract() != 0.0 || *f < lo as f64 || *f > hi as f64 {
                return Err(Trap::new(format!(
                    "numeric cast failed: cannot convert {f} to {}: value is out of range or not an integer",
                    to.name()
                )));
            }
            *f as i128
        }
        Val::Int(_) => {
            let x = int_of(from, v)?;
            if x < lo || x > hi {
                return Err(Trap::new(format!(
                    "numeric cast failed: cannot convert {x} to {}: value is out of range",
                    to.name()
                )));
            }
            x
        }
        other => return Err(bad("cast of a non-number", other)),
    };
    int_to(to, wide)
}

// ------------------------------------------------------------ projections

/// A projection with its register operands already read.
pub(crate) enum Key {
    Field(u32),
    Tuple(u32),
    Index(usize),
    Key(String),
}

fn resolve_proj(proj: &Proj, regs: &[Val]) -> Res<Key> {
    Ok(match proj {
        Proj::Field(i) => Key::Field(*i),
        Proj::Tuple(i) => Key::Tuple(*i),
        // The bound is checked against the container when it is known.
        Proj::Index(r) => Key::Index(match &regs[r.0 as usize] {
            Val::Float(f) if *f >= 0.0 && f.fract() == 0.0 => *f as usize,
            Val::Int(i) if *i >= 0 => *i as usize,
            other => {
                return Err(Trap::new(format!(
                    "index must be a non-negative integer, got {other:?}"
                )))
            }
        }),
        Proj::Key(r) => match &regs[r.0 as usize] {
            Val::Str(s) => Key::Key(s.to_string()),
            other => return Err(bad("map key must be a string", other)),
        },
    })
}

fn get_proj(base: &Val, proj: &Proj, regs: &[Val]) -> Res<Val> {
    let key = resolve_proj(proj, regs)?;
    match (base, key) {
        (Val::Adt(a), Key::Field(i)) => Ok(a.fields[i as usize].clone()),
        (Val::Tuple(t), Key::Tuple(i)) => Ok(t[i as usize].clone()),
        (Val::List(l), Key::Index(i)) => l
            .get(i)
            .cloned()
            .ok_or_else(|| Trap::new(format!("index {i} out of bounds (len={})", l.len()))),
        (Val::Map(m), Key::Key(k)) => m
            .get(&k)
            .cloned()
            .ok_or_else(|| Trap::new(format!("key not found: {k:?}"))),
        (other, _) => Err(bad("projection of the wrong kind of value", other)),
    }
}

fn take_proj(base: &mut Val, key: Key) -> Res<Val> {
    let slot = slot_mut(base, key)?;
    Ok(std::mem::replace(slot, Val::Undef))
}

fn set_proj(base: &mut Val, key: Key, value: Val) -> Res<()> {
    if let (Val::Map(m), Key::Key(k)) = (&mut *base, &key) {
        Rc::make_mut(m).insert(k.clone(), value);
        return Ok(());
    }
    *slot_mut(base, key)? = value;
    Ok(())
}

fn slot_mut(base: &mut Val, key: Key) -> Res<&mut Val> {
    match (base, key) {
        (Val::Adt(a), Key::Field(i)) => Ok(&mut Rc::make_mut(a).fields[i as usize]),
        (Val::Tuple(t), Key::Tuple(i)) => Ok(&mut Rc::make_mut(t)[i as usize]),
        (Val::List(l), Key::Index(i)) => {
            let len = l.len();
            Rc::make_mut(l)
                .get_mut(i)
                .ok_or_else(|| Trap::new(format!("index {i} out of bounds (len={len})")))
        }
        (Val::Map(m), Key::Key(k)) => Rc::make_mut(m)
            .get_mut(&k)
            .ok_or_else(|| Trap::new(format!("key not found: {k:?}"))),
        (other, _) => Err(bad("projection of the wrong kind of value", other)),
    }
}
