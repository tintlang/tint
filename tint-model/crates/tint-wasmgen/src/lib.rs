//! WebAssembly backend for the typed IR.
//!
//! The output is an `app` module that imports the memory and the functions of
//! the runtime module (`tint-wasmrt`, module name `rt`) and exports the entry
//! functions. When instantiated it copies its static data (module descriptor,
//! string literals) into the runtime's memory, creates the literal objects and
//! runs the global initializers.
//!
//! Value mapping: `bool` and `unit` are `i32`, every integer kind an `i64` (the
//! same convention as the interpreter: `u64` as its bit pattern, narrower kinds
//! sign- or zero-extended), every float kind an `f64` (`f32` values are rounded
//! after each operation), every heap value an `i32` pointer (0 is empty).
//! Ownership follows `tint-codegen`: a heap register holds null or one owned
//! reference.
//!
//! Anything the backend cannot compile is reported as `Unsupported` at compile
//! time, never miscompiled.

mod cfg;
mod func;

use func::{Cx, ExtraKey, Imp};
use std::collections::HashMap;
use tint_ir::typed::*;
pub use tint_wasmabi::{kind_index, KINDS};
use tint_wasmabi::{encode, is_heap};
use wasm_encoder::{
    CodeSection, ConstExpr, DataCountSection, DataSection, ElementSection, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    Instruction as I, MemoryType, Module as WasmModule, RefType, StartSection, TableSection,
    TableType, TypeSection, ValType,
};

#[derive(Debug, PartialEq)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported by the wasm backend: {}", self.0)
    }
}

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Result<T, Unsupported> {
    Err(Unsupported(what.into()))
}

pub struct Compiled {
    /// The `app` module.
    pub wasm: Vec<u8>,
    /// Texts of the traps (also in the module's static data).
    pub messages: Vec<String>,
}

/// What a register or value of type `ty` becomes in wasm.
pub(crate) fn val_type(m: &Module, ty: TyId) -> ValType {
    match m.types.kind(ty) {
        TyKind::Unit | TyKind::Bool => ValType::I32,
        TyKind::Num(k) if k.is_float() => ValType::F64,
        TyKind::Num(_) => ValType::I64,
        _ => ValType::I32,
    }
}

/// Functions reachable from `roots` through calls and closures, in discovery
/// order, and which of them are closure bodies.
fn reachable(m: &Module, roots: &[FuncId]) -> (Vec<FuncId>, Vec<bool>) {
    let mut seen = vec![false; m.funcs.len()];
    let mut targets = vec![false; m.funcs.len()];
    let mut order = Vec::new();
    let mut stack: Vec<FuncId> = roots.iter().rev().copied().collect();
    while let Some(id) = stack.pop() {
        if std::mem::replace(&mut seen[id.0 as usize], true) {
            continue;
        }
        order.push(id);
        for block in &m.func(id).blocks {
            for ins in &block.instrs {
                match ins {
                    Instr::Call { func, .. } => stack.push(*func),
                    Instr::Closure { func, .. } => {
                        targets[func.0 as usize] = true;
                        stack.push(*func);
                    }
                    _ => {}
                }
            }
        }
    }
    (order, targets)
}

/// Compiles the functions named in `entries` and everything they reach. Each
/// entry is exported under its name.
pub fn compile(module: &Module, entries: &[&str]) -> Result<Compiled, Unsupported> {
    compile_with(module, entries, "")
}

/// Like `compile`; `meta` (JSON of the program's `app { }` metadata) is stored
/// in the module's descriptor for the UI runtime.
pub fn compile_with(module: &Module, entries: &[&str], meta: &str) -> Result<Compiled, Unsupported> {
    // The first pass finds out which runtime functions are used; the second
    // emits the final module with only those imported.
    let all = vec![true; Imp::COUNT];
    let first = build(module, entries, &all, meta)?;
    build(module, entries, &first.used, meta).map(|b| b.compiled)
}

/// Compiles a UI app: the `ui fn`s, their event handlers, and the functions the
/// host calls (`tint:set_<var>`, `tint:enter:<ui fn>`), all exported by name.
pub fn compile_app(module: &Module, ui_fns: &[&str], meta: &str) -> Result<Compiled, Unsupported> {
    let mut entries: Vec<String> = ui_fns.iter().map(|s| s.to_string()).collect();
    entries.extend(ui_handlers(&module.ui_templates, &module.functions));
    // `http_get` completions: `fn on_x(id: number, status: number, body: string)`.
    let mut names: Vec<&String> = module.functions.keys().collect();
    names.sort();
    for name in names {
        let f = module.func(module.functions[name]);
        if !matches!(f.kind, FuncKind::Fn) || f.params.len() != 3 {
            continue;
        }
        let kinds: Vec<&TyKind> = f.params.iter().map(|r| module.types.kind(f.reg_ty(*r))).collect();
        if matches!(kinds[..], [TyKind::Num(a), TyKind::Num(b), TyKind::Str] if a.is_float() && b.is_float()) && !entries.contains(name) {
            entries.push(name.clone());
        }
    }
    for name in ["theme", "viewport_width", "route_path"] {
        entries.push(format!("tint:set_{name}"));
    }
    for name in ui_fns {
        entries.push(format!("tint:enter:{name}"));
    }
    // Page routes may name more ui fns than the entry.
    let entries: Vec<&str> = entries.iter().map(String::as_str).filter(|n| module.functions.contains_key(*n)).collect();
    compile_with(module, &entries, meta)
}

struct Built {
    compiled: Compiled,
    used: Vec<bool>,
}

fn build(module: &Module, entries: &[&str], import_mask: &[bool], meta: &str) -> Result<Built, Unsupported> {
    let mut roots = Vec::new();
    let mut unique: Vec<&str> = Vec::new();
    for name in entries {
        if !unique.contains(name) {
            unique.push(name);
        }
    }
    let entries = &unique[..];
    for name in entries {
        match module.functions.get(*name) {
            Some(id) => roots.push(*id),
            None => return unsupported(format!("no function `{name}`")),
        }
    }
    let entry_ids = roots.clone();
    if let Some(init) = module.init {
        roots.push(init);
    }
    let (order, targets) = reachable(module, &roots);
    let thunked: Vec<FuncId> = order.iter().copied().filter(|id| targets[id.0 as usize]).collect();

    let mut cx = Cx::new(module, import_mask);
    let nimports = cx.nimports();
    let index: HashMap<FuncId, u32> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, nimports + i as u32))
        .collect();
    let thunk_index: HashMap<FuncId, u32> = thunked
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, nimports + order.len() as u32 + i as u32))
        .collect();
    cx.set_indices(index.clone());

    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();
    for id in &order {
        let f = module.func(*id);
        if !matches!(f.kind, FuncKind::Fn | FuncKind::Init | FuncKind::Method { .. } | FuncKind::Lambda | FuncKind::Ui) {
            return unsupported(format!("`{}` ({:?})", f.name, f.kind));
        }
        let params: Vec<ValType> = f.params.iter().map(|r| val_type(module, f.reg_ty(*r))).collect();
        let ty = cx.func_type(params, vec![val_type(module, f.ret)]);
        funcs.function(ty);
        let body = func::compile_func(&mut cx, f).map_err(|e| Unsupported(format!("in `{}`: {}", f.name, e.0)))?;
        code.function(&body);
    }
    for id in &thunked {
        let f = module.func(*id);
        let mut params = vec![ValType::I32];
        params.extend(f.params[f.ncaptures as usize..].iter().map(|r| val_type(module, f.reg_ty(*r))));
        let ty = cx.func_type(params, vec![val_type(module, f.ret)]);
        funcs.function(ty);
        code.function(&func::thunk(&mut cx, f, index[id]));
    }

    // Module-level pieces that depend on everything compiled so far.
    let msgs = cx.messages();
    let sigs: Vec<(String, String)> = entries
        .iter()
        .zip(&entry_ids)
        .map(|(name, id)| {
            let f = module.func(*id);
            let kinds = f
                .params
                .iter()
                .map(|r| match val_type(module, f.reg_ty(*r)) {
                    ValType::F64 => 'f',
                    ValType::I64 => 'i',
                    _ => 'p',
                })
                .collect();
            (name.to_string(), kinds)
        })
        .collect();
    let desc = encode(module, &msgs, meta, &sigs);
    let nmodule_globals = module.globals.len() as u32;

    // Static data: the descriptor, then the string literals.
    let mut data = desc.clone();
    let mut lit_offsets = HashMap::new();
    for (i, e) in cx.extras().iter().enumerate() {
        if let ExtraKey::Lit(s) = e {
            lit_offsets.insert(i, (data.len() as u32 - desc.len() as u32, s.len() as u32));
            data.extend_from_slice(s.as_bytes());
        }
    }
    let callback_index = nimports + (order.len() + thunked.len()) as u32;
    let has_callbacks = !module.natives.is_empty();
    if has_callbacks {
        let ty = cx.func_type(vec![ValType::I32, ValType::I32], vec![]);
        funcs.function(ty);
        code.function(&func::run_callback(&mut cx));
    }
    let start = func::start(&mut cx, desc.len() as u32, data.len() as u32, &lit_offsets, module.init.map(|i| index[&i]));
    let start_index = nimports + (order.len() + thunked.len()) as u32 + has_callbacks as u32;
    let start_ty = cx.func_type(vec![], vec![]);
    funcs.function(start_ty);
    code.function(&start);

    let mut out = WasmModule::new();
    let mut types = TypeSection::new();
    for (params, results) in cx.type_list() {
        types.ty().function(params.clone(), results.clone());
    }
    out.section(&types);
    let mut imports = ImportSection::new();
    imports.import(
        "rt",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 0,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    for (name, ty) in cx.import_list() {
        imports.import("rt", name, EntityType::Function(ty));
    }
    out.section(&imports);
    out.section(&funcs);
    let mut table = TableSection::new();
    table.table(TableType {
        element_type: RefType::FUNCREF,
        minimum: module.funcs.len() as u64,
        maximum: None,
        table64: false,
        shared: false,
    });
    out.section(&table);
    let mut globals = GlobalSection::new();
    for g in &module.globals {
        let vt = val_type(module, g.ty);
        let init = match vt {
            ValType::F64 => ConstExpr::f64_const(0.0.into()),
            ValType::I64 => ConstExpr::i64_const(0),
            _ => ConstExpr::i32_const(0),
        };
        globals.global(GlobalType { val_type: vt, mutable: true, shared: false }, &init);
    }
    for _ in cx.extras() {
        globals.global(
            GlobalType { val_type: ValType::I32, mutable: true, shared: false },
            &ConstExpr::i32_const(0),
        );
    }
    let _ = nmodule_globals;
    out.section(&globals);
    let mut exports = ExportSection::new();
    for (name, id) in entries.iter().zip(&entry_ids) {
        exports.export(name, ExportKind::Func, index[id]);
    }
    if has_callbacks {
        exports.export("tint:run_callback", ExportKind::Func, callback_index);
    }
    out.section(&exports);
    out.section(&StartSection { function_index: start_index });
    let mut elems = ElementSection::new();
    for id in &thunked {
        elems.active(
            Some(0),
            &ConstExpr::i32_const(id.0 as i32),
            wasm_encoder::Elements::Functions(std::borrow::Cow::Borrowed(&[thunk_index[id]])),
        );
    }
    out.section(&elems);
    out.section(&DataCountSection { count: 1 });
    out.section(&code);
    let mut data_section = DataSection::new();
    data_section.passive(data);
    out.section(&data_section);
    let used = cx.used_imports();
    let _ = (is_heap, Function::new([]), I::Nop);
    Ok(Built { compiled: Compiled { wasm: out.finish(), messages: msgs }, used })
}
