//! `lower_errs entry.tn...`: resolve + check + lower + wasm, errors per file.
use tint_ir::typed::lower_program;

fn main() {
    for path in std::env::args().skip(1) {
        let r = std::panic::catch_unwind(|| one(&path));
        if r.is_err() { println!("{path}: PANIC"); }
    }
}

fn one(path: &str) {
    let resolved = match tint_compiler::resolver::resolve_path(path) {
        Ok(r) => r,
        Err(e) => { println!("{path}: RESOLVE {e}"); return; }
    };
    let (errors, model) = resolved.check();
    if !errors.is_empty() { println!("{path}: CHECK {} errors, first {:?}", errors.len(), errors[0]); return; }
    let lowered = lower_program(&resolved.program, &model);
    if !lowered.errors.is_empty() {
        println!("{path}: LOWER {} errors", lowered.errors.len());
        for e in lowered.errors.iter().take(8) { println!("    {e:?}"); }
        return;
    }
    println!("{path}: ok ({} funcs, {} templates)", lowered.module.funcs.len(), lowered.module.ui_templates.len());
}
