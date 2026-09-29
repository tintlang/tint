//! Lowering: typed AST + `SemanticModel` -> typed IR `Module`.
//!
//! The checker decides every type; lowering never infers. What it does decide:
//! how untyped `number` values meet sized numbers (literals take the type of
//! their context, other values are converted with a checked cast), how closures
//! capture, where a method must hand its receiver back, and how `match`, `?`,
//! loops and the collection helpers turn into blocks.

mod analysis;
mod builder;
mod calls;
mod expr;
mod pattern;
mod stmt;
mod ui;

use super::ir::*;
use super::ty::*;
use super::verify;
use builder::FnB;
use std::collections::{HashMap, HashSet};
use tint_ast::{EnumDecl, FnBody, FnDecl, Item, Pattern, Program, Span, Stmt, StructDecl, UiFnDecl};
use tint_semantics::{SemanticModel, Type};

#[derive(Debug, Clone)]
pub struct LowerError {
    pub message: String,
    pub span: Option<Span>,
    /// The function or item being lowered.
    pub item: String,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.span {
            Some(span) if span.start.line > 0 => write!(
                f,
                "{}:{}: in `{}`: {}",
                span.start.line, span.start.column, self.item, self.message
            ),
            _ => write!(f, "in `{}`: {}", self.item, self.message),
        }
    }
}

pub(crate) type LResult<T> = Result<T, LowerError>;

/// Result of lowering a program.
#[derive(Debug)]
pub struct Lowered {
    pub module: Module,
    /// Items that could not be lowered. Their functions exist in the module
    /// but only trap.
    pub errors: Vec<LowerError>,
    /// Items left out on purpose (kernels: nothing defines what they mean yet).
    pub skipped: Vec<String>,
}

pub(crate) struct StructInfo {
    pub generics: Vec<String>,
    pub fields: Vec<String>,
}

pub(crate) struct VariantInfo {
    pub name: String,
    pub names: Vec<Option<String>>,
}

pub(crate) struct EnumInfo {
    pub generics: Vec<String>,
    pub variants: Vec<VariantInfo>,
}

#[derive(Clone)]
pub(crate) struct FnInfo<'a> {
    pub id: FuncId,
    pub params: Vec<TyId>,
    pub ret: TyId,
    pub sigs: Vec<tint_ast::ParamSig>,
    /// The default of each parameter, evaluated at the call site.
    pub defaults: Vec<Option<&'a tint_ast::Expr>>,
}

#[derive(Clone)]
pub(crate) struct MethodInfo {
    pub id: FuncId,
    /// Without the receiver.
    pub params: Vec<TyId>,
    /// What the method returns to its caller (before the `(result, self)`
    /// wrapping of `inout_self` methods).
    pub ret: TyId,
    pub inout_self: bool,
}

pub(crate) struct Lowerer<'a> {
    pub model: &'a SemanticModel,
    pub module: Module,
    pub structs: HashMap<String, StructInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub fns: HashMap<String, FnInfo<'a>>,
    /// Generic functions, instantiated on demand (see `fn_instance`).
    pub generic_fns: HashMap<String, &'a FnDecl>,
    pub instances: HashMap<(String, Vec<TyId>), FnInfo<'a>>,
    /// Instances whose bodies are still to be lowered.
    pub pending: Vec<FnJob<'a>>,
    pub methods: HashMap<(String, String), MethodInfo>,
    pub globals: HashMap<String, GlobalId>,
    pub stack: Vec<FnB>,
    pub lambdas: usize,
    /// Generic parameter bindings while an ADT instance is being built.
    pub subst: Vec<HashMap<String, TyId>>,
    pub item: String,
}

/// Lowers every logic item of `program`. `model` must come from checking this
/// very `program` value (expression types are keyed by node address).
pub fn lower_program<'a>(program: &'a Program, model: &'a SemanticModel) -> Lowered {
    let mut lowerer = Lowerer {
        model,
        module: Module::default(),
        structs: HashMap::new(),
        enums: HashMap::new(),
        fns: HashMap::new(),
        generic_fns: HashMap::new(),
        instances: HashMap::new(),
        pending: Vec::new(),
        methods: HashMap::new(),
        globals: HashMap::new(),
        stack: Vec::new(),
        lambdas: 0,
        subst: Vec::new(),
        item: String::new(),
    };
    let mut errors = Vec::new();
    let mut skipped = Vec::new();
    lowerer.run(program, &mut errors, &mut skipped);
    let module = lowerer.module;
    for problem in verify::verify(&module) {
        errors.push(LowerError { message: format!("invalid IR: {problem}"), span: None, item: "<module>".into() });
    }
    Lowered { module, errors, skipped }
}

fn flatten<'p>(items: &'p [Item], out: &mut Vec<&'p Item>) {
    for item in items {
        match item {
            Item::Mod(m) => flatten(&m.items, out),
            Item::Space(s) => flatten(&s.items, out),
            other => out.push(other),
        }
    }
}

/// A function or method that has to be lowered.
pub(crate) struct FnJob<'p> {
    decl: &'p FnDecl,
    owner: Option<String>,
    id: FuncId,
    /// An instance of a generic function: its signature and what its type
    /// parameters stand for.
    instance: Option<(FnInfo<'p>, HashMap<String, TyId>)>,
}

impl<'a> Lowerer<'a> {
    pub fn err<T>(&self, span: Option<Span>, message: impl Into<String>) -> LResult<T> {
        Err(LowerError { message: message.into(), span, item: self.item.clone() })
    }

    pub fn error(&self, span: Option<Span>, message: impl Into<String>) -> LowerError {
        LowerError { message: message.into(), span, item: self.item.clone() }
    }

    fn run(&mut self, program: &'a Program, errors: &mut Vec<LowerError>, skipped: &mut Vec<String>) {
        let mut items = Vec::new();
        flatten(&program.items, &mut items);

        // Declarations of types.
        for item in &items {
            match item {
                Item::Struct(s) | Item::ExportStruct(s) => self.declare_struct(s),
                Item::Enum(e) | Item::ExportEnum(e) => self.declare_enum(e),
                _ => {}
            }
        }

        // Globals: constants and top-level lets.
        let mut global_inits: Vec<&Item> = Vec::new();
        for item in &items {
            match item {
                Item::Const(c) => {
                    self.item = c.name.clone();
                    match self.declare_global(&c.name) {
                        Ok(()) => global_inits.push(item),
                        Err(e) => errors.push(e),
                    }
                }
                Item::GlobalLet(Stmt::Let { pattern, .. }) => {
                    self.item = "<global>".into();
                    let mut names = Vec::new();
                    pattern_names(pattern, &mut names);
                    let mut ok = true;
                    for name in names {
                        if let Err(e) = self.declare_global(&name) {
                            errors.push(e);
                            ok = false;
                        }
                    }
                    if ok {
                        global_inits.push(item);
                    }
                }
                Item::UiFn(f) => {
                    self.item = f.name.clone();
                    if let Err(e) = self.declare_ui_globals(f) {
                        errors.push(e);
                    }
                }
                _ => {}
            }
        }

        // Signatures of functions and methods.
        let mut jobs: Vec<FnJob> = Vec::new();
        let mut ui_jobs: Vec<(&'a UiFnDecl, FuncId)> = Vec::new();
        for item in &items {
            match item {
                Item::Fn(f) | Item::ExportFn(f, _) => {
                    self.item = f.name.clone();
                    if !f.generics.is_empty() {
                        self.generic_fns.insert(f.name.clone(), f);
                        continue;
                    }
                    match self.declare_fn(f) {
                        Ok(id) => jobs.push(FnJob { decl: f, owner: None, id, instance: None }),
                        Err(e) => errors.push(e),
                    }
                }
                Item::UiFn(f) => {
                    self.item = f.name.clone();
                    match self.declare_ui_fn(f) {
                        Ok(id) => ui_jobs.push((f, id)),
                        Err(e) => errors.push(e),
                    }
                }
                Item::Kernel(k) => skipped.push(format!("kernel {} (no semantics defined in the checker or the tree-walker)", k.name)),
                _ => {}
            }
        }
        let mutating = self.mutating_methods(&items);
        for item in &items {
            if let Item::Impl(block) = item {
                let owner = match &block.target {
                    tint_ast::Type::Simple(name) if block.generics.is_empty() => name.clone(),
                    other => {
                        self.item = format!("impl {other:?}");
                        errors.push(self.error(Some(block.span), "only `impl Name { .. }` on a plain struct can be lowered"));
                        continue;
                    }
                };
                for method in &block.methods {
                    self.item = format!("{owner}.{}", method.name);
                    let inout = mutating.contains(&(owner.clone(), method.name.clone()));
                    match self.declare_method(&owner, method, inout) {
                        Ok(id) => jobs.push(FnJob { decl: method, owner: Some(owner.clone()), id, instance: None }),
                        Err(e) => errors.push(e),
                    }
                }
            }
        }

        for (f, id) in &ui_jobs {
            self.item = f.name.clone();
            if let Err(e) = self.lower_ui_fn(f, *id) {
                self.stack.clear();
                self.fail_function(*id, &e);
                errors.push(e);
            }
        }

        // Bodies. Lowering a call of a generic function may add instances.
        loop {
            if jobs.is_empty() {
                jobs.append(&mut self.pending);
                if jobs.is_empty() {
                    break;
                }
            }
            let job = jobs.remove(0);
            self.item = match (&job.owner, &job.instance) {
                (Some(owner), _) => format!("{owner}.{}", job.decl.name),
                (None, Some(_)) => self.module.funcs[job.id.0 as usize].name.clone(),
                (None, None) => job.decl.name.clone(),
            };
            let depth = self.subst.len();
            if let Err(e) = self.lower_fn(&job) {
                self.stack.clear();
                self.subst.truncate(depth);
                self.fail_function(job.id, &e);
                errors.push(e);
            }
        }

        // Global initializers.
        if !self.module.globals.is_empty() {
            self.item = "<globals>".into();
            if let Err(e) = self.lower_init(&global_inits, &ui_jobs.iter().map(|(f, _)| *f).collect::<Vec<_>>()) {
                self.stack.clear();
                errors.push(e);
            }
        }
    }

    fn fail_function(&mut self, id: FuncId, error: &LowerError) {
        let func = &mut self.module.funcs[id.0 as usize];
        func.blocks = vec![Block {
            instrs: Vec::new(),
            term: Term::Trap(format!("`{}` could not be lowered: {}", func.name, error.message)),
        }];
    }

    // ------------------------------------------------------------ types

    fn declare_struct(&mut self, s: &StructDecl) {
        let fields = s
            .members
            .iter()
            .filter_map(|m| match m {
                tint_ast::StructMember::Field(f) => Some(match f {
                    tint_ast::StructField::Typed { name, .. }
                    | tint_ast::StructField::TintTyped { name, .. }
                    | tint_ast::StructField::TintField { name, .. } => name.clone(),
                }),
                tint_ast::StructMember::Kit(_) => None,
            })
            .collect();
        self.structs.insert(s.name.clone(), StructInfo { generics: s.generics.clone(), fields });
    }

    fn declare_enum(&mut self, e: &EnumDecl) {
        let variants = e
            .variants
            .iter()
            .map(|v| match v {
                tint_ast::EnumVariant::Unit(name) => VariantInfo { name: name.clone(), names: Vec::new() },
                tint_ast::EnumVariant::Tuple(name, tys) => {
                    VariantInfo { name: name.clone(), names: vec![None; tys.len()] }
                }
                tint_ast::EnumVariant::Struct(name, fields) => VariantInfo {
                    name: name.clone(),
                    names: fields
                        .iter()
                        .map(|f| {
                            Some(match f {
                                tint_ast::StructField::Typed { name, .. }
                                | tint_ast::StructField::TintTyped { name, .. }
                                | tint_ast::StructField::TintField { name, .. } => name.clone(),
                            })
                        })
                        .collect(),
                },
            })
            .collect();
        self.enums.insert(e.name.clone(), EnumInfo { generics: e.generics.clone(), variants });
    }

    /// Converts a checker type. Fails on anything without a run-time shape
    /// (`Unknown`, UI types, unbound generic parameters).
    pub fn conv(&mut self, ty: &Type) -> LResult<TyId> {
        Ok(match ty {
            Type::Unit => self.module.types.unit(),
            Type::Bool => self.module.types.bool(),
            Type::String => self.module.types.str(),
            Type::Number => self.module.types.num(NumKind::Num),
            Type::Simple(name) => {
                if let Some(bound) = self.subst.last().and_then(|s| s.get(name)) {
                    return Ok(*bound);
                }
                match NumKind::from_name(name) {
                    Some(kind) => self.module.types.num(kind),
                    None => return self.err(None, format!("type `{name}` has no run-time representation")),
                }
            }
            Type::Array(item) => {
                let item = self.conv(item)?;
                self.module.types.list(item)
            }
            Type::Map(item) => {
                let item = self.conv(item)?;
                self.module.types.map(item)
            }
            Type::Tuple(items) => {
                let items = items.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
                self.module.types.tuple(items)
            }
            Type::Fn(ret, params) => {
                let params = params.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
                let ret = self.conv(ret)?;
                self.module.types.func(params, ret)
            }
            Type::Struct(name) | Type::Enum(name) => {
                let adt = self.adt_instance(name, Vec::new())?;
                self.module.types.adt_ty(adt)
            }
            Type::Generic(name, args) => {
                let args = args.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
                let adt = self.adt_instance(name, args)?;
                self.module.types.adt_ty(adt)
            }
            other => return self.err(None, format!("type {other:?} has no run-time representation")),
        })
    }

    /// The monomorphized struct or enum `base<args>`.
    pub fn adt_instance(&mut self, base: &str, args: Vec<TyId>) -> LResult<AdtId> {
        let (id, fresh) = self.module.types.declare_adt(base, &args);
        if !fresh {
            return Ok(id);
        }
        let named = |name: &str, ty: TyId| FieldDef { name: name.to_string(), ty };
        let body = match base {
            "Option" | "Result" if args.len() == if base == "Option" { 1 } else { 2 } => {
                let unit_variant = |name: &str| VariantDef { name: name.into(), fields: vec![], positional: false };
                let one = |name: &str, field: &str, ty: TyId| VariantDef {
                    name: name.into(),
                    fields: vec![named(field, ty)],
                    positional: false,
                };
                AdtBody::Enum(if base == "Option" {
                    vec![unit_variant("None"), one("Some", "value", args[0])]
                } else {
                    vec![one("Ok", "value", args[0]), one("Err", "error", args[1])]
                })
            }
            "Vec2" => {
                let num = self.module.types.num(NumKind::Num);
                AdtBody::Struct(vec![named("x", num), named("y", num)])
            }
            _ if self.structs.contains_key(base) => {
                let generics = self.structs[base].generics.clone();
                let names = self.structs[base].fields.clone();
                if generics.len() != args.len() {
                    return self.err(None, format!("`{base}` needs {} type arguments", generics.len()));
                }
                self.subst.push(generics.into_iter().zip(args.iter().copied()).collect());
                let result = names
                    .iter()
                    .map(|name| {
                        let ty = self
                            .model
                            .struct_fields
                            .get(base)
                            .and_then(|f| f.get(name))
                            .cloned()
                            .ok_or_else(|| self.error(None, format!("no type recorded for `{base}.{name}`")))?;
                        Ok(FieldDef { name: name.clone(), ty: self.conv(&ty)? })
                    })
                    .collect::<LResult<Vec<_>>>();
                self.subst.pop();
                AdtBody::Struct(result?)
            }
            _ if self.enums.contains_key(base) => {
                let generics = self.enums[base].generics.clone();
                if generics.len() != args.len() {
                    return self.err(None, format!("`{base}` needs {} type arguments", generics.len()));
                }
                let variants: Vec<(String, Vec<Option<String>>)> = self.enums[base]
                    .variants
                    .iter()
                    .map(|v| (v.name.clone(), v.names.clone()))
                    .collect();
                self.subst.push(generics.into_iter().zip(args.iter().copied()).collect());
                let result = variants
                    .iter()
                    .map(|(name, names)| {
                        let payload = self
                            .model
                            .variant_types
                            .get(&(base.to_string(), name.clone()))
                            .cloned()
                            .unwrap_or_default();
                        let mut fields = Vec::new();
                        for (i, ty) in payload.iter().enumerate() {
                            let field_name = names.get(i).cloned().flatten().unwrap_or_else(|| i.to_string());
                            fields.push(FieldDef { name: field_name, ty: self.conv(ty)? });
                        }
                        Ok(VariantDef {
                            name: name.clone(),
                            fields,
                            positional: names.iter().all(|n| n.is_none()),
                        })
                    })
                    .collect::<LResult<Vec<_>>>();
                self.subst.pop();
                AdtBody::Enum(result?)
            }
            _ => return self.err(None, format!("unknown struct or enum `{base}`")),
        };
        self.module.types.set_adt_body(id, body);
        Ok(id)
    }

    // ------------------------------------------------------ declarations

    fn declare_global(&mut self, name: &str) -> LResult<()> {
        let ty = match self.model.globals.get(name) {
            Some(ty) => ty.clone(),
            None => return self.err(None, format!("no type recorded for `{name}`")),
        };
        let ty = self.conv(&ty)?;
        let id = GlobalId(self.module.globals.len() as u32);
        self.module.globals.push(Global { name: name.to_string(), ty });
        self.globals.insert(name.to_string(), id);
        Ok(())
    }

    fn reserve_func(&mut self, name: &str, kind: FuncKind, ret: TyId) -> FuncId {
        let id = FuncId(self.module.funcs.len() as u32);
        self.module.funcs.push(Func {
            name: name.to_string(),
            kind,
            params: Vec::new(),
            ncaptures: 0,
            ret,
            regs: Vec::new(),
            blocks: vec![Block { instrs: Vec::new(), term: Term::Trap(format!("`{name}` was not lowered")) }],
        });
        id
    }

    fn declare_fn(&mut self, f: &'a FnDecl) -> LResult<FuncId> {
        let (params, ret) = match self.model.functions.get(&f.name) {
            Some(sig) => sig.clone(),
            None => return self.err(Some(f.span), "no signature recorded"),
        };
        let params = params.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
        let ret = self.conv(&ret)?;
        let id = self.reserve_func(&f.name, FuncKind::Fn, ret);
        self.module.functions.insert(f.name.clone(), id);
        let sigs = f.params.iter().map(|p| p.sig()).collect();
        let defaults = f.params.iter().map(|p| p.default.as_ref().map(|d| d.expr())).collect();
        self.fns.insert(f.name.clone(), FnInfo { id, params, ret, sigs, defaults });
        Ok(id)
    }

    fn declare_method(&mut self, owner: &str, f: &FnDecl, inout_self: bool) -> LResult<FuncId> {
        if !f.generics.is_empty() {
            return self.err(Some(f.span), "generic methods cannot be lowered yet");
        }
        if !f.params.first().is_some_and(analysis::is_self_param) {
            return self.err(Some(f.span), "method without `self` cannot be lowered");
        }
        let key = (owner.to_string(), f.name.clone());
        let (params, ret) = match self.model.methods.get(&key) {
            Some(sig) => sig.clone(),
            None => return self.err(Some(f.span), "no signature recorded"),
        };
        let params = params.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
        let ret = self.conv(&ret)?;
        let full_ret = if inout_self {
            let self_ty = self.conv(&Type::Struct(owner.to_string()))?;
            self.module.types.tuple(vec![ret, self_ty])
        } else {
            ret
        };
        let name = format!("{owner}.{}", f.name);
        let id = self.reserve_func(&name, FuncKind::Method { inout_self }, full_ret);
        self.module.methods.insert(key.clone(), id);
        self.methods.insert(key, MethodInfo { id, params, ret, inout_self });
        Ok(id)
    }

    // ------------------------------------------------------------ bodies

    fn lower_fn(&mut self, job: &FnJob) -> LResult<()> {
        let decl = job.decl;
        let info_ret;
        let mut param_tys: Vec<TyId> = Vec::new();
        let mut inout = false;
        match &job.owner {
            Some(owner) => {
                let info = self.methods[&(owner.clone(), decl.name.clone())].clone();
                inout = info.inout_self;
                info_ret = info.ret;
                let self_ty = self.conv(&Type::Struct(owner.clone()))?;
                param_tys.push(self_ty);
                param_tys.extend(info.params.iter().copied());
            }
            None => {
                let info = match &job.instance {
                    Some((info, _)) => info.clone(),
                    None => self.fns[&decl.name].clone(),
                };
                info_ret = info.ret;
                param_tys.extend(info.params.iter().copied());
            }
        }
        if let Some((_, subst)) = &job.instance {
            self.subst.push(subst.clone());
        }
        let kind = match &job.owner {
            Some(_) => FuncKind::Method { inout_self: inout },
            None => FuncKind::Fn,
        };
        let name = self.module.funcs[job.id.0 as usize].name.clone();
        self.stack.push(FnB::new(name, kind, info_ret));
        self.f().inout = inout;

        if decl.params.len() != param_tys.len() {
            return self.err(Some(decl.span), "parameter count differs from the recorded signature");
        }
        let mut destructure: Vec<(usize, Reg)> = Vec::new();
        for (index, (param, ty)) in decl.params.iter().zip(&param_tys).enumerate() {
            let reg = self.new_reg(*ty);
            self.f().params.push(reg);
            match strip_pattern(&param.pattern) {
                Pattern::Ident(name, _) => {
                    self.f().define(name, reg);
                    if name == "self" {
                        self.f().self_reg = Some(reg);
                    }
                }
                _ => destructure.push((index, reg)),
            }
        }
        for (index, reg) in destructure {
            let ty = param_tys[index];
            let fail = self.trap_block("parameter pattern did not match");
            self.bind_pattern(&decl.params[index].pattern, reg, ty, fail)?;
        }

        let ret = info_ret;
        let value = match &decl.body {
            FnBody::Block(block) => self.block(block, Some(ret))?,
            FnBody::Expr(e) => self.expr(e, Some(ret))?,
        };
        self.finish_return(value, decl.span)?;

        let fb = self.stack.pop().unwrap();
        let func = fb.finish(self.module.funcs[job.id.0 as usize].ret);
        self.module.funcs[job.id.0 as usize] = func;
        if job.instance.is_some() {
            self.subst.pop();
        }
        Ok(())
    }

    /// The instance of generic function `name` for the type arguments `targs`
    /// (in the order of its type parameters). Created, and queued for
    /// lowering, the first time it is asked for.
    pub fn fn_instance(&mut self, name: &str, targs: Vec<TyId>, span: Span) -> LResult<FnInfo<'a>> {
        let key = (name.to_string(), targs);
        if let Some(info) = self.instances.get(&key) {
            return Ok(info.clone());
        }
        const LIMIT: usize = 1024;
        if self.instances.len() >= LIMIT {
            return self.err(Some(span), format!("more than {LIMIT} instances of generic functions (does `{name}` instantiate itself with ever larger types?)"));
        }
        let Some(decl) = self.generic_fns.get(name).copied() else {
            return self.err(Some(span), format!("`{name}` is not a generic function"));
        };
        let (generics, sig) = match (self.model.function_generics.get(name), self.model.functions.get(name)) {
            (Some(g), Some(sig)) => (g.clone(), sig.clone()),
            _ => return self.err(Some(span), format!("no signature recorded for `{name}`")),
        };
        if generics.len() != key.1.len() {
            return self.err(Some(span), format!("`{name}` takes {} type arguments, got {}", generics.len(), key.1.len()));
        }
        let subst: HashMap<String, TyId> = generics.into_iter().zip(key.1.iter().copied()).collect();
        self.subst.push(subst.clone());
        let converted = (|| -> LResult<(Vec<TyId>, TyId)> {
            let params = sig.0.iter().map(|t| self.conv(t)).collect::<LResult<Vec<_>>>()?;
            Ok((params, self.conv(&sig.1)?))
        })();
        self.subst.pop();
        let (params, ret) = converted?;
        let shown: Vec<String> = key.1.iter().map(|t| self.show(*t)).collect();
        let mangled = format!("{name}<{}>", shown.join(", "));
        let id = self.reserve_func(&mangled, FuncKind::Fn, ret);
        self.module.functions.insert(mangled, id);
        let sigs = decl.params.iter().map(|p| p.sig()).collect();
        let defaults = decl.params.iter().map(|p| p.default.as_ref().map(|d| d.expr())).collect();
        let info = FnInfo { id, params, ret, sigs, defaults };
        self.instances.insert(key, info.clone());
        self.pending.push(FnJob { decl, owner: None, id, instance: Some((info.clone(), subst)) });
        Ok(info)
    }

    fn lower_init(&mut self, inits: &[&Item], ui_fns: &[&'a UiFnDecl]) -> LResult<()> {
        let unit = self.module.types.unit();
        let id = self.reserve_func("<init>", FuncKind::Init, unit);
        self.stack.push(FnB::new("<init>".into(), FuncKind::Init, unit));
        for item in inits {
            match item {
                Item::Const(c) => {
                    self.item = c.name.clone();
                    let global = self.globals[&c.name];
                    let ty = self.module.globals[global.0 as usize].ty;
                    let value = self.expr_as(&c.init, ty)?;
                    self.emit(Instr::GlobalSet { global, src: value });
                }
                Item::GlobalLet(Stmt::Let { pattern, ty: declared, init, span }) => {
                    self.item = "<global>".into();
                    let declared = match declared {
                        Some(t) => Some(self.ast_ty(t)?),
                        None => None,
                    };
                    let value = match declared {
                        Some(t) => self.expr_as(init.expr(), t)?,
                        None => self.expr(init.expr(), None)?,
                    };
                    let ty = self.f().reg_ty(value);
                    let fail = self.trap_block("global pattern did not match");
                    self.push_scope();
                    self.bind_pattern(pattern, value, ty, fail)?;
                    let mut names = Vec::new();
                    pattern_names(pattern, &mut names);
                    for name in names {
                        let reg = self.f().lookup_local(&name).ok_or_else(|| self.error(Some(*span), "unbound"))?;
                        let global = self.globals[&name];
                        let want = self.module.globals[global.0 as usize].ty;
                        let reg = self.coerce(reg, want, Some(*span))?;
                        self.emit(Instr::GlobalSet { global, src: reg });
                    }
                    self.pop_scope();
                }
                _ => {}
            }
        }
        self.item = "<ui state>".into();
        self.lower_ui_state_inits(ui_fns)?;
        let value = self.unit_reg();
        self.terminate(Term::Return(value));
        let fb = self.stack.pop().unwrap();
        self.module.funcs[id.0 as usize] = fb.finish(unit);
        self.module.init = Some(id);
        Ok(())
    }

    /// Converts a type written in source (`let x: i32`, casts).
    pub fn ast_ty(&mut self, ty: &tint_ast::Type) -> LResult<TyId> {
        let converted = match ty {
            tint_ast::Type::Simple(name) => match name.as_str() {
                "()" | "unit" => Type::Unit,
                "string" | "str" => Type::String,
                "bool" => Type::Bool,
                "number" => Type::Number,
                n if NumKind::from_name(n).is_some() => Type::Simple(n.to_string()),
                n if self.enums.contains_key(n) => Type::Enum(n.to_string()),
                n => Type::Struct(n.to_string()),
            },
            tint_ast::Type::Generic(name, args) if name == "Vec" || name == "Array" => {
                let item = args.first().ok_or_else(|| self.error(None, "Vec needs an element type"))?;
                let item = self.ast_ty(item)?;
                return Ok(self.module.types.list(item));
            }
            tint_ast::Type::Generic(name, args) => {
                let args = args.iter().map(|a| self.ast_ty(a)).collect::<LResult<Vec<_>>>()?;
                let adt = self.adt_instance(name, args)?;
                return Ok(self.module.types.adt_ty(adt));
            }
            tint_ast::Type::Unit => Type::Unit,
            tint_ast::Type::Union(items) => {
                let items = items.iter().map(|a| self.ast_ty(a)).collect::<LResult<Vec<_>>>()?;
                return Ok(self.module.types.tuple(items));
            }
            tint_ast::Type::Function { params, ret } => {
                let params = params.iter().map(|a| self.ast_ty(a)).collect::<LResult<Vec<_>>>()?;
                let ret = self.ast_ty(ret)?;
                return Ok(self.module.types.func(params, ret));
            }
        };
        self.conv(&converted)
    }
}

/// `mut x`, `x: i32` -> the pattern underneath.
pub(crate) fn strip_pattern(p: &Pattern) -> &Pattern {
    match p {
        Pattern::Mut { inner, .. } => strip_pattern(inner),
        Pattern::Typed { pat, .. } => strip_pattern(pat),
        other => other,
    }
}

pub(crate) fn pattern_names(p: &Pattern, out: &mut Vec<String>) {
    match p {
        Pattern::Ident(name, _) => out.push(name.clone()),
        Pattern::Mut { inner, .. } => pattern_names(inner, out),
        Pattern::Typed { pat, .. } => pattern_names(pat, out),
        Pattern::Tuple(items, _) => items.iter().for_each(|i| pattern_names(i, out)),
        Pattern::Variant { args, .. } => args.iter().for_each(|i| pattern_names(i, out)),
        Pattern::Struct { fields, .. } | Pattern::Map { fields, .. } | Pattern::Group { fields, .. } => {
            for f in fields {
                match f {
                    tint_ast::PatternField::Shorthand { field, .. } => out.push(field.clone()),
                    tint_ast::PatternField::Assign { pat, .. } => pattern_names(pat, out),
                    tint_ast::PatternField::Rest(_) => {}
                }
            }
        }
        Pattern::Number(..) | Pattern::String(..) | Pattern::Wildcard(_) => {}
    }
}

#[allow(dead_code)]
fn _unused(_: HashSet<()>) {}
