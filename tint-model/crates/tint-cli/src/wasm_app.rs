//! `tint build` (default engine): the program compiled to a WebAssembly module
//! (`tint-wasmgen`) that runs on the embedded `tint-wasmrt` runtime.

use tint_ast::Item;
use tint_ir::typed::{lower_program, UiTemplate};

use crate::module_loader::Loaded;

pub(crate) struct CompiledApp {
    pub wasm: Vec<u8>,
    /// Every `ui fn`, in source order.
    pub ui_fns: Vec<String>,
    /// The program uses `Preview`, which runs source text at run time.
    pub needs_interpreter: bool,
}

/// Compiles the program; anything the backend cannot compile is an error naming it.
pub(crate) fn compile(loaded: &Loaded) -> Result<CompiledApp, String> {
    let (_, model) = loaded.resolved.check();
    let lowered = lower_program(&loaded.resolved.program, &model);
    if !lowered.errors.is_empty() {
        let lines: Vec<String> = lowered
            .errors
            .iter()
            .map(|e| match e.span {
                Some(span) => format!("  {} (line {}): {}", e.item, span.start.line, e.message),
                None => format!("  {}: {}", e.item, e.message),
            })
            .collect();
        return Err(format!("cannot compile to WebAssembly:\n{}", lines.join("\n")));
    }
    let program = &loaded.resolved.program;
    let ui_fns: Vec<String> = program
        .items
        .iter()
        .filter_map(|item| if let Item::UiFn(f) = item { Some(f.name.clone()) } else { None })
        .collect();
    if ui_fns.is_empty() {
        return Err("nothing to build: the program has no `ui fn`".to_string());
    }
    let needs_interpreter = lowered
        .module
        .ui_templates
        .iter()
        .any(|t| matches!(t, UiTemplate::Element(e) if e.tag == "Preview"));
    let meta = serde_json::to_string(&program.app_meta()).map_err(|e| e.to_string())?;
    let names: Vec<&str> = ui_fns.iter().map(String::as_str).collect();
    let compiled = tint_wasmgen::compile_app(&lowered.module, &names, &meta).map_err(|e| e.to_string())?;
    Ok(CompiledApp { wasm: compiled.wasm, ui_fns, needs_interpreter })
}
