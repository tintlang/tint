//! Text dump of a module, for tests and debugging.

use super::ir::*;
use super::ui::*;
use std::fmt::Write;

fn cmp_name(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "eq",
        CmpOp::Ne => "ne",
        CmpOp::Lt => "lt",
        CmpOp::Le => "le",
        CmpOp::Gt => "gt",
        CmpOp::Ge => "ge",
    }
}

fn bin_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
    }
}

fn regs(list: &[Reg]) -> String {
    list.iter().map(|r| format!("r{}", r.0)).collect::<Vec<_>>().join(", ")
}

fn proj(p: &Proj) -> String {
    match p {
        Proj::Field(i) => format!(".{i}"),
        Proj::Tuple(i) => format!(".{i}"),
        Proj::Index(r) => format!("[r{}]", r.0),
        Proj::Key(r) => format!("{{r{}}}", r.0),
    }
}

pub fn show_const(value: &Const) -> String {
    match value {
        Const::Unit => "()".into(),
        Const::Bool(b) => b.to_string(),
        Const::Int(i) => i.to_string(),
        Const::Float(f) => format!("{f:?}"),
        Const::Str(s) => format!("{s:?}"),
    }
}

pub fn show_instr(module: &Module, func: &Func, instr: &Instr) -> String {
    let types = &module.types;
    let name_of = |id: FuncId| module.funcs[id.0 as usize].name.clone();
    match instr {
        Instr::Const { dst, value } => format!("r{} = const {}", dst.0, show_const(value)),
        Instr::Mov { dst, src } => format!("r{} = mov r{}", dst.0, src.0),
        Instr::Bin { dst, op, kind, a, b } => {
            format!("r{} = {}.{} r{}, r{}", dst.0, bin_name(*op), kind.name(), a.0, b.0)
        }
        Instr::Cmp { dst, op, a, b } => format!("r{} = {} r{}, r{}", dst.0, cmp_name(*op), a.0, b.0),
        Instr::Not { dst, src } => format!("r{} = not r{}", dst.0, src.0),
        Instr::Neg { dst, src } => format!("r{} = neg r{}", dst.0, src.0),
        Instr::Cast { dst, src } => {
            format!("r{} = cast.{} r{}", dst.0, types.show(func.reg_ty(*dst)), src.0)
        }
        Instr::ToStr { dst, src } => format!("r{} = to_str r{}", dst.0, src.0),
        Instr::Tuple { dst, items } => format!("r{} = tuple({})", dst.0, regs(items)),
        Instr::Struct { dst, adt, fields } => {
            format!("r{} = struct {}({})", dst.0, types.show_adt(*adt), regs(fields))
        }
        Instr::Variant { dst, adt, variant, fields } => {
            let def = types.adt(*adt);
            format!(
                "r{} = variant {}::{}({})",
                dst.0,
                types.show_adt(*adt),
                def.variants()[*variant as usize].name,
                regs(fields)
            )
        }
        Instr::List { dst, items } => format!("r{} = list[{}]", dst.0, regs(items)),
        Instr::Map { dst, entries } => {
            let entries: Vec<String> = entries.iter().map(|(k, r)| format!("{k:?}: r{}", r.0)).collect();
            format!("r{} = map{{{}}}", dst.0, entries.join(", "))
        }
        Instr::Get { dst, base, proj: p } => format!("r{} = get r{}{}", dst.0, base.0, proj(p)),
        Instr::Take { dst, base, proj: p } => format!("r{} = take r{}{}", dst.0, base.0, proj(p)),
        Instr::Set { base, proj: p, src } => format!("set r{}{} = r{}", base.0, proj(p), src.0),
        Instr::Tag { dst, src } => format!("r{} = tag r{}", dst.0, src.0),
        Instr::Payload { dst, src, variant, index } => {
            format!("r{} = payload r{} #{}.{}", dst.0, src.0, variant, index)
        }
        Instr::Rt { dst, f, args } => format!("r{} = rt {}({})", dst.0, f.name(), regs(args)),
        Instr::Host { dst, f, args } => format!("r{} = host {}({})", dst.0, f.name(), regs(args)),
        Instr::Call { dst, func: callee, args } => {
            format!("r{} = call {}({})", dst.0, name_of(*callee), regs(args))
        }
        Instr::CallClosure { dst, callee, args } => {
            format!("r{} = call_closure r{}({})", dst.0, callee.0, regs(args))
        }
        Instr::Closure { dst, func: callee, captures } => {
            format!("r{} = closure {}[{}]", dst.0, name_of(*callee), regs(captures))
        }
        Instr::GlobalGet { dst, global } => {
            format!("r{} = global {}", dst.0, module.globals[global.0 as usize].name)
        }
        Instr::GlobalSet { global, src } => {
            format!("global {} = r{}", module.globals[global.0 as usize].name, src.0)
        }
        Instr::UiOpen { template, values } => {
            let tag = match &module.ui_templates[*template as usize] {
                UiTemplate::Element(e) => e.tag.as_str(),
                UiTemplate::Tokens { .. } => "<tokens>",
            };
            format!("ui.open {tag} #{template}({})", regs(values))
        }
        Instr::UiClose => "ui.close".to_string(),
        Instr::UiText { src } => format!("ui.text r{}", src.0),
        Instr::UiTokens { template } => format!("ui.tokens #{template}"),
        Instr::UiMemo { dst, site, inputs } => format!("r{} = ui.memo #{site}({})", dst.0, regs(inputs)),
        Instr::UiMemoEnd => "ui.memo_end".to_string(),
    }
}

pub fn show_term(term: &Term) -> String {
    match term {
        Term::Jump(b) => format!("jump b{}", b.0),
        Term::Branch { cond, then_, else_ } => {
            format!("branch r{} ? b{} : b{}", cond.0, then_.0, else_.0)
        }
        Term::Switch { value, cases, default } => {
            let cases: Vec<String> = cases.iter().map(|(v, b)| format!("{v} => b{}", b.0)).collect();
            format!("switch r{} [{}] default b{}", value.0, cases.join(", "), default.0)
        }
        Term::Return(r) => format!("ret r{}", r.0),
        Term::Trap(msg) => format!("trap {msg:?}"),
    }
}

pub fn show_func(module: &Module, func: &Func) -> String {
    let types = &module.types;
    let mut out = String::new();
    let params: Vec<String> = func
        .params
        .iter()
        .map(|p| format!("r{}: {}", p.0, types.show(func.reg_ty(*p))))
        .collect();
    let _ = writeln!(out, "fn {}({}) -> {} {{", func.name, params.join(", "), types.show(func.ret));
    for (index, block) in func.blocks.iter().enumerate() {
        let _ = writeln!(out, "  b{index}:");
        for instr in &block.instrs {
            let _ = writeln!(out, "    {}", show_instr(module, func, instr));
        }
        let _ = writeln!(out, "    {}", show_term(&block.term));
    }
    out.push_str("}\n");
    out
}

impl std::fmt::Display for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for global in &self.globals {
            writeln!(f, "global {}: {}", global.name, self.types.show(global.ty))?;
        }
        for func in &self.funcs {
            write!(f, "{}", show_func(self, func))?;
        }
        Ok(())
    }
}
