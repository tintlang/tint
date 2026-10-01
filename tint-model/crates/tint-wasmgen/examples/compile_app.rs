//! `compile_app entry.tn out.wasm [UiFn]`: compiles the UI app of `entry.tn`
//! (every `ui fn` is exported; the first, or `UiFn`, is rendered first by the host).
use tint_ir::typed::lower_program;
use tint_ast::Item;

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, out) = (args.next().unwrap(), args.next().unwrap());
    let resolved = tint_compiler::resolver::resolve_path(&path).unwrap_or_else(|e| panic!("{e}"));
    let (errors, model) = resolved.check();
    for e in &errors {
        eprintln!("warning: semantic: {e:?}");
    }
    let lowered = lower_program(&resolved.program, &model);
    if !lowered.errors.is_empty() {
        for e in &lowered.errors {
            eprintln!("error: {}: {}", e.item, e.message);
        }
        std::process::exit(1);
    }
    let ui_fns: Vec<&str> = resolved
        .program
        .items
        .iter()
        .filter_map(|i| if let Item::UiFn(f) = i { Some(f.name.as_str()) } else { None })
        .collect();
    let meta = serde_json::to_string(&resolved.program.app_meta()).unwrap();
    let c = tint_wasmgen::compile_app(&lowered.module, &ui_fns, &meta).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });
    std::fs::write(&out, &c.wasm).unwrap();
    if let Ok(bc) = tint_runtime::bytecode::encode(&resolved.program) {
        std::fs::write(format!("{out}.bc"), bc).unwrap();
    }
    eprintln!("{} bytes, ui fns: {}", c.wasm.len(), ui_fns.join(" "));
}
