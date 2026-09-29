//! Module discovery and name resolution for Tint source files.
//!
//! This is intentionally independent of the CLI.  It produces the same flat
//! `Program` shape the runtime currently consumes, while retaining a module
//! table internally so the future semantic model can stop flattening names.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use tint_ast::{Item, ModDecl, Program, UseDecl};

use crate::{expand_source_imports, SourceDatabase};

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
struct Module {
    path: Vec<String>,
    items: Vec<Item>,
}

type ModuleTree = HashMap<Vec<String>, Module>;

/// Loads an entry file, discovers its `mod` tree, and resolves reachable
/// `use` declarations into the flat program consumed by today's runtime.
pub fn resolve_path(entry_path: impl AsRef<Path>) -> Result<ResolvedProgram, String> {
    resolve_path_with_overlays(entry_path, &HashMap::new())
}

pub fn resolve_path_with_overlays(
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
    active.insert(root_path.clone());
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
        program: Program {
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

fn flatten(tree: &ModuleTree) -> Result<Vec<Item>, String> {
    let root = tree
        .get(&Vec::<String>::new())
        .ok_or_else(|| "resolver produced no root module".to_string())?;
    let mut merged: HashMap<String, (Vec<String>, Item)> = HashMap::new();
    let mut order = Vec::new();

    for item in &root.items {
        if let Some((name, _)) = export_info(item) {
            insert(
                &mut merged,
                &mut order,
                name.to_string(),
                vec![],
                item.clone(),
            )?;
        }
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([Vec::<String>::new()]);
    while let Some(module_path) = queue.pop_front() {
        if !visited.insert(module_path.clone()) {
            continue;
        }
        let Some(module) = tree.get(&module_path) else {
            continue;
        };
        for item in &module.items {
            if let Item::Use(use_decl) = item {
                resolve_use(tree, module, use_decl, &mut merged, &mut order, &mut queue)?;
            }
        }
    }

    let mut output = root
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Use(_)) && export_info(item).is_none())
        .cloned()
        .collect::<Vec<_>>();
    output.extend(
        order
            .into_iter()
            .map(|name| merged.remove(&name).unwrap().1),
    );
    Ok(output)
}

fn resolve_use(
    tree: &ModuleTree,
    importer: &Module,
    declaration: &UseDecl,
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    queue: &mut VecDeque<Vec<String>>,
) -> Result<(), String> {
    if declaration.wildcard {
        let target = declaration.path.clone();
        let module = tree.get(&target).ok_or_else(|| {
            format!(
                "`use {}` in `{}` -- no module `{}`",
                use_string(declaration),
                path_string(&importer.path),
                path_string(&target)
            )
        })?;
        for item in &module.items {
            if let Some((name, true)) = export_info(item) {
                insert(
                    merged,
                    order,
                    name.to_string(),
                    target.clone(),
                    item.clone(),
                )?;
            }
        }
        queue.push_back(target);
        return Ok(());
    }
    if declaration.path.len() < 2 {
        return Err(format!(
            "`use {}` needs `module::item`",
            use_string(declaration)
        ));
    }
    let split = declaration.path.len() - 1;
    let module_path = declaration.path[..split].to_vec();
    let item_name = &declaration.path[split];
    let bound_name = declaration
        .alias
        .clone()
        .unwrap_or_else(|| item_name.clone());
    let module = tree.get(&module_path).ok_or_else(|| {
        format!(
            "`use {}` in `{}` -- no module `{}`",
            use_string(declaration),
            path_string(&importer.path),
            path_string(&module_path)
        )
    })?;
    let found = module.items.iter().find_map(|item| {
        export_info(item)
            .filter(|(name, _)| *name == item_name)
            .map(|(_, exported)| (item, exported))
    });
    match found {
        Some((item, true)) => {
            let item = declaration
                .alias
                .as_deref()
                .map_or_else(|| item.clone(), |alias| rename_item(item.clone(), alias));
            insert(merged, order, bound_name, module_path.clone(), item)?;
            queue.push_back(module_path);
            Ok(())
        }
        Some((_, false)) => Err(format!(
            "`use {}` in `{}` -- `{}` exists in `{}` but isn't exported (add `export` before it)",
            use_string(declaration),
            path_string(&importer.path),
            item_name,
            path_string(&module_path)
        )),
        None => Err(format!(
            "`use {}` in `{}` -- no exported item named `{}` in `{}`",
            use_string(declaration),
            path_string(&importer.path),
            item_name,
            path_string(&module_path)
        )),
    }
}

fn insert(
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    name: String,
    origin: Vec<String>,
    item: Item,
) -> Result<(), String> {
    if let Some((existing, _)) = merged.get(&name) {
        if *existing == origin {
            return Ok(());
        }
        return Err(format!(
            "`{}` is defined in both `{}` and `{}`",
            name,
            path_string(existing),
            path_string(&origin)
        ));
    }
    order.push(name.clone());
    merged.insert(name, (origin, item));
    Ok(())
}

fn export_info(item: &Item) -> Option<(&str, bool)> {
    match item {
        Item::Fn(f) => Some((&f.name, f.exported)),
        Item::ExportFn(f, _) => Some((&f.name, true)),
        Item::Struct(s) => Some((&s.name, false)),
        Item::ExportStruct(s) => Some((&s.name, true)),
        Item::Enum(e) => Some((&e.name, false)),
        Item::ExportEnum(e) => Some((&e.name, true)),
        _ => None,
    }
}

fn rename_item(item: Item, name: &str) -> Item {
    match item {
        Item::Fn(mut f) => {
            f.name = name.into();
            Item::Fn(f)
        }
        Item::ExportFn(mut f, span) => {
            f.name = name.into();
            Item::ExportFn(f, span)
        }
        Item::Struct(mut s) => {
            s.name = name.into();
            Item::Struct(s)
        }
        Item::ExportStruct(mut s) => {
            s.name = name.into();
            Item::ExportStruct(s)
        }
        Item::Enum(mut e) => {
            e.name = name.into();
            Item::Enum(e)
        }
        Item::ExportEnum(mut e) => {
            e.name = name.into();
            Item::ExportEnum(e)
        }
        other => other,
    }
}

fn source_text(sources: &SourceDatabase, id: crate::SourceId) -> Result<String, String> {
    sources
        .source(id)
        .map(|source| source.text.clone())
        .ok_or_else(|| "source disappeared".into())
}

fn append(path: &[String], name: &str) -> Vec<String> {
    let mut result = path.to_vec();
    result.push(name.to_string());
    result
}

fn path_string(path: &[String]) -> String {
    if path.is_empty() {
        "<entry file>".into()
    } else {
        path.join("::")
    }
}

fn use_string(declaration: &UseDecl) -> String {
    let base = declaration.path.join("::");
    if declaration.wildcard {
        format!("{}::*", base)
    } else if let Some(alias) = &declaration.alias {
        format!("{} as {}", base, alias)
    } else {
        base
    }
}

fn canonical_or_absolute(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
