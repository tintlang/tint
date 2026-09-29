//! Module discovery and name resolution for Tint source files.
//!
//! The implementation is split by responsibility: [`modules`] discovers and
//! loads the module tree, [`imports`] resolves `use` declarations, and
//! [`utils`] contains shared path and diagnostic helpers.

use std::collections::HashMap;

use tint_ast::{Item, Program};

#[path = "imports.rs"]
mod imports;
#[path = "modules.rs"]
mod modules;
#[path = "utils.rs"]
mod utils;

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub entry_source: String,
    pub program: Program,
    pub all_sources: Vec<String>,
}

impl ResolvedProgram {
    pub fn context(&self) -> tint_semantics::SemanticContext {
        tint_semantics::SemanticChecker::context_for_program(&self.program)
    }

    /// Checks the resolved program with the compiler's shared semantic pass.
    /// Keeping this entry point here makes the resolver/type-checker pipeline
    /// available to the CLI and future analyzer without duplicating setup.
    pub fn check(
        &self,
    ) -> (
        Vec<tint_semantics::SemanticError>,
        tint_semantics::SemanticModel,
    ) {
        let context = self.context();
        let mut checker = tint_semantics::SemanticChecker::new(Default::default());
        checker.check_with_context(&self.program, &context)
    }
}

#[derive(Debug, Clone)]
pub(super) struct Module {
    pub(super) path: Vec<String>,
    pub(super) items: Vec<Item>,
}

pub(super) type ModuleTree = HashMap<Vec<String>, Module>;

/// Loads an entry file, discovers its `mod` tree, and resolves reachable
/// `use` declarations into the flat program consumed by today's runtime.
pub fn resolve_path(entry_path: impl AsRef<std::path::Path>) -> Result<ResolvedProgram, String> {
    modules::resolve_path_with_overlays(entry_path, &HashMap::new())
}

pub fn resolve_path_with_overlays(
    entry_path: impl AsRef<std::path::Path>,
    overlays: &HashMap<std::path::PathBuf, String>,
) -> Result<ResolvedProgram, String> {
    modules::resolve_path_with_overlays(entry_path, overlays)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn fixture(name: &str) -> PathBuf {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("tint-resolver-{name}-{id}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write(root: &Path, name: &str, text: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn names(program: &ResolvedProgram) -> Vec<String> {
        program
            .program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(f) | Item::ExportFn(f, _) => Some(f.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn resolves_exported_items_and_aliases() {
        let root = fixture("alias");
        write(&root, "math.tn", "export fn add(a, b) { a + b }\n");
        let main = write(
            &root,
            "main.tn",
            "mod math; use math::add as plus; fn main() { plus(1, 2) }\n",
        );
        let resolved = resolve_path(&main).unwrap();
        let names = names(&resolved);
        assert!(names.contains(&"plus".to_string()));
        assert!(!names.contains(&"add".to_string()));
    }

    #[test]
    fn rejects_private_imports() {
        let root = fixture("private");
        write(&root, "math.tn", "fn add(a, b) { a + b }\n");
        let main = write(
            &root,
            "main.tn",
            "mod math; use math::add; fn main() { 1 }\n",
        );
        let error = resolve_path(&main).unwrap_err();
        assert!(error.contains("isn't exported"));
    }
}
