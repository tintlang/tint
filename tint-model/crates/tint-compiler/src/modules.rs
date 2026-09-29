use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tint_ast::{Item, ModDecl};

use crate::{expand_source_imports, SourceDatabase};

use super::imports::flatten;
use super::utils::{append, canonical_or_absolute, path_string, source_text};
use super::{Module, ModuleTree, ResolvedProgram};

pub(super) fn resolve_path_with_overlays(
    entry_path: impl AsRef<Path>,
    overlays: &HashMap<PathBuf, String>,
) -> Result<ResolvedProgram, String> {
    let entry_path = entry_path.as_ref();
    let mut sources = SourceDatabase::new();
    let entry_id = load_source(&mut sources, entry_path, overlays)?;
    let entry_source = source_text(&sources, entry_id)?;
    let entry_program = sources
        .parse(entry_id)
        .map_err(|error| format!("parse error in '{}': {:?}", entry_path.display(), error))?;

    let root_path = canonical_or_absolute(entry_path);
    let entry_dir = root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut tree = ModuleTree::new();
    let mut active = HashSet::new();
    active.insert(root_path);
    let mut all_sources = vec![entry_source.clone()];

    collect_modules(
        &entry_dir,
        Vec::new(),
        entry_program.items,
        &mut sources,
        &mut tree,
        &mut active,
        &mut all_sources,
        overlays,
    )?;

    let items = flatten(&tree)?;
    Ok(ResolvedProgram {
        entry_source,
        program: tint_ast::Program {
            globals: entry_program.globals,
            items,
        },
        all_sources,
    })
}

fn collect_modules(
    dir: &Path,
    path: Vec<String>,
    items: Vec<Item>,
    sources: &mut SourceDatabase,
    tree: &mut ModuleTree,
    active: &mut HashSet<PathBuf>,
    all_sources: &mut Vec<String>,
    overlays: &HashMap<PathBuf, String>,
) -> Result<(), String> {
    let mut own_items = Vec::new();
    for item in items {
        match item {
            Item::Mod(module) if module.external => {
                let child_path = append(&path, &module.name);
                let (child_dir, child_file, child_items, source) =
                    load_external(dir, &path, &module, sources, overlays)?;
                let canonical = canonical_or_absolute(&child_file);
                if !active.insert(canonical.clone()) {
                    return Err(format!(
                        "circular `mod` reference through `{}` (`{}` is already active)",
                        path_string(&child_path),
                        canonical.display()
                    ));
                }
                all_sources.push(source);
                collect_modules(
                    &child_dir,
                    child_path,
                    child_items,
                    sources,
                    tree,
                    active,
                    all_sources,
                    overlays,
                )?;
                active.remove(&canonical);
            }
            Item::Mod(module) => {
                let child_path = append(&path, &module.name);
                collect_modules(
                    dir,
                    child_path,
                    module.items,
                    sources,
                    tree,
                    active,
                    all_sources,
                    overlays,
                )?;
            }
            other => own_items.push(other),
        }
    }

    tree.insert(
        path.clone(),
        Module {
            path,
            items: own_items,
        },
    );
    Ok(())
}

fn load_external(
    dir: &Path,
    parent: &[String],
    declaration: &ModDecl,
    sources: &mut SourceDatabase,
    overlays: &HashMap<PathBuf, String>,
) -> Result<(PathBuf, PathBuf, Vec<Item>, String), String> {
    let flat = dir.join(format!("{}.tn", declaration.name));
    let nested = dir.join(&declaration.name).join("mod.tn");
    let (file, child_dir) = if flat.is_file() {
        (flat, dir.to_path_buf())
    } else if nested.is_file() {
        (nested, dir.join(&declaration.name))
    } else {
        return Err(format!(
            "cannot find module `{}` -- looked for `{}` and `{}`",
            path_string(&append(parent, &declaration.name)),
            flat.display(),
            nested.display()
        ));
    };

    let id = load_source(sources, &file, overlays)?;
    let source = source_text(sources, id)?;
    let program = sources
        .parse(id)
        .map_err(|error| format!("parse error in '{}': {:?}", file.display(), error))?;
    Ok((child_dir, file, program.items, source))
}

fn load_source(
    sources: &mut SourceDatabase,
    path: &Path,
    overlays: &HashMap<PathBuf, String>,
) -> Result<crate::SourceId, String> {
    let normalized = canonical_or_absolute(path);
    let text = expand_source_imports(&normalized, overlays)?;
    Ok(sources.set_source(normalized, text))
}
