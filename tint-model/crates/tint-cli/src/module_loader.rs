//! CLI adapter for the compiler module resolver.
//!
//! Module discovery and import resolution live in `tint-compiler`. The CLI
//! only preserves the historical `Loaded` shape and process-exit behavior.

use tint_ast::Program;
use tint_compiler::resolver::{resolve_path, ResolvedProgram};

pub(crate) struct Loaded {
    pub entry_source: String,
    pub program: Program,
    pub all_sources: Vec<String>,
    pub resolved: ResolvedProgram,
}

pub(crate) fn load(entry_path: &str) -> Loaded {
    match try_load(entry_path) {
        Ok(loaded) => loaded,
        Err(message) => {
            eprintln!("module error: {}", message);
            std::process::exit(1);
        }
    }
}

pub(crate) fn try_load(entry_path: &str) -> Result<Loaded, String> {
    resolve_path(entry_path).map(|resolved| Loaded {
        entry_source: resolved.entry_source.clone(),
        program: resolved.program.clone(),
        all_sources: resolved.all_sources.clone(),
        resolved,
    })
}
