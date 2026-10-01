/// Compiles `main` and everything it reaches.
pub fn compile(module: Module) -> Result<Jit, Unsupported> {
    compile_for(module, &["main"])
}

/// Compiles the functions named in `entries` (and the globals initializer)
/// and everything they reach. Functions nothing reaches are not looked at, so
/// a module may contain code the backend cannot handle.
pub fn compile_for(module: Module, entries: &[&str]) -> Result<Jit, Unsupported> {
    let module = Box::new(module);
    let mut roots: Vec<FuncId> = Vec::new();
    for name in entries {
        match module.functions.get(*name) {
            Some(id) => roots.push(*id),
            None => return unsupported(format!("no function `{name}`")),
        }
    }
    let (mut wanted, mut targets, uses_globals) = reachable(&module, &roots);
    if uses_globals {
        if let Some(init) = module.init {
            let (w, t, _) = reachable(&module, &[init]);
            for i in 0..wanted.len() {
                wanted[i] |= w[i];
                targets[i] |= t[i];
            }
        }
    }
    // Closure bodies are entered through `fn_table` (filled after linking)
    // and globals live in `global_slots`; generated code embeds both addresses.
    let fn_table: Box<[usize]> = vec![0; module.funcs.len()].into_boxed_slice();
    let global_slots: Box<[u64]> = vec![0; module.globals.len()].into_boxed_slice();

    let mut flags = settings::builder();
    flags.set("opt_level", "speed").unwrap();
    flags.set("preserve_frame_pointers", "false").unwrap();
    let isa = cranelift_native::builder()
        .map_err(|e| Unsupported(e.to_string()))?
        .finish(settings::Flags::new(flags))
        .map_err(|e| Unsupported(e.to_string()))?;
    let mut jb = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let table = import_table();
    for spec in &table {
        jb.symbol(format!("tint_rt_{}", spec.name), spec.ptr);
    }
    let mut jit = JITModule::new(jb);
    let mut import_ids = Vec::new();
    for spec in &table {
        let mut sig = jit.make_signature();
        for p in &spec.params {
            sig.params.push(AbiParam::new(*p));
        }
        if let Some(r) = spec.ret {
            sig.returns.push(AbiParam::new(r));
        }
        let id = jit
            .declare_function(&format!("tint_rt_{}", spec.name), Linkage::Import, &sig)
            .unwrap();
        import_ids.push((spec.name, id));
    }

    // Declare every wanted function first so calls can be forward references.
    let mut ids: Vec<Option<ClifFunc>> = vec![None; module.funcs.len()];
    for (i, f) in module.funcs.iter().enumerate() {
        if !wanted[i] {
            continue;
        }
        if !matches!(
            f.kind,
            FuncKind::Fn | FuncKind::Init | FuncKind::Method { .. } | FuncKind::Lambda
        ) {
            return unsupported(format!("function `{}` of kind {:?}", f.name, f.kind));
        }
        let mut sig = jit.make_signature();
        for p in &f.params {
            sig.params
                .push(AbiParam::new(clif_ty(&module, f.reg_ty(*p))?));
        }
        sig.returns.push(AbiParam::new(clif_ty(&module, f.ret)?));
        ids[i] = Some(
            jit.declare_function(&format!("f{i}_{}", f.name), Linkage::Local, &sig)
                .unwrap(),
        );
    }

    let mut msgs: Vec<String> = Vec::new();
    let mut fctx = FunctionBuilderContext::new();
    for (i, f) in module.funcs.iter().enumerate() {
        let Some(id) = ids[i] else { continue };
        let mut ctx = jit.make_context();
        ctx.func.signature = jit.declarations().get_function_decl(id).signature.clone();
        {
            let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
            let imports: HashMap<&'static str, _> = import_ids
                .iter()
                .map(|(name, id)| (*name, jit.declare_func_in_func(*id, b.func)))
                .collect();
            let callees: Vec<Option<_>> = ids
                .iter()
                .map(|c| c.map(|c| jit.declare_func_in_func(c, b.func)))
                .collect();
            lower::lower_func(
                &module,
                f,
                &mut b,
                &imports,
                &callees,
                &mut msgs,
                fn_table.as_ptr() as i64,
                global_slots.as_ptr() as i64,
            )
            .map_err(|e| Unsupported(format!("in `{}`: {}", f.name, e.0)))?;
            b.seal_all_blocks();
            b.finalize(jit.target_config());
        }
        jit.define_function(id, &mut ctx)
            .map_err(|e| Unsupported(format!("cranelift: {e}")))?;
        jit.clear_context(&mut ctx);
    }
    // Uniform entry points for closure bodies: (closure, declared params...).
    let mut thunks: Vec<(usize, ClifFunc)> = Vec::new();
    for (i, f) in module.funcs.iter().enumerate() {
        if !targets[i] {
            continue;
        }
        let Some(real) = ids[i] else { continue };
        let mut sig = jit.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        for p in &f.params[f.ncaptures as usize..] {
            sig.params
                .push(AbiParam::new(clif_ty(&module, f.reg_ty(*p))?));
        }
        sig.returns.push(AbiParam::new(clif_ty(&module, f.ret)?));
        let id = jit
            .declare_function(&format!("thunk{i}_{}", f.name), Linkage::Local, &sig)
            .unwrap();
        let mut ctx = jit.make_context();
        ctx.func.signature = sig;
        {
            let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
            let real_ref = jit.declare_func_in_func(real, b.func);
            let retain = jit.declare_func_in_func(
                import_ids.iter().find(|(n, _)| *n == "retain").unwrap().1,
                b.func,
            );
            lower::thunk_body(&module, f, &mut b, real_ref, retain);
            b.seal_all_blocks();
            b.finalize(jit.target_config());
        }
        jit.define_function(id, &mut ctx)
            .map_err(|e| Unsupported(format!("cranelift: {e}")))?;
        jit.clear_context(&mut ctx);
        thunks.push((i, id));
    }
    jit.finalize_definitions()
        .map_err(|e| Unsupported(e.to_string()))?;
    let mut fn_table = fn_table;
    for (i, id) in thunks {
        fn_table[i] = jit.get_finalized_function(id) as usize;
    }
    Ok(Jit {
        jit,
        ids,
        module,
        msgs,
        _fn_table: fn_table,
        _globals: global_slots,
        init_done: std::cell::Cell::new(false),
    })
}
