use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use tint_ast::{Item, ModDecl, Program, UseDecl};

use crate::support::{parse_source, read_file, report_parse_error};

/// A module's own items, keyed by its path from the entry file's root
/// module (`[]` is the entry file itself, `["math"]` is what `mod math;`
/// resolves to, `["math", "vec3"]` is `mod vec3;` declared inside *that*
/// file, and so on). `Item::Mod` nodes never appear as values here -- they
/// are stripped out while the tree is built and turned into more entries
/// in this same map instead.
type ModuleTree = HashMap<Vec<String>, Vec<Item>>;

/// What loading an entry file (and everything it transitively `mod`s in)
/// produces.
#[derive(Debug)]
pub(crate) struct Loaded {
    /// The entry file's own raw source text -- what parse/semantic error
    /// spans for code written directly in the entry file are offsets
    /// into. (A parse error inside a *submodule* file is caught and
    /// reported, against that file's own text, the moment that file is
    /// read -- see `load_external_module` -- so it never reaches here. A
    /// semantic error inside code pulled in from a submodule still prints
    /// against this text, which will show the wrong snippet; see `load`'s
    /// doc comment.)
    pub entry_source: String,
    /// The entry file's own items, plus every item actually reachable
    /// through a `use` chain -- one flat program, ready for the semantic
    /// checker / `register_functions` / `run_program`, exactly like a
    /// single-file program always has.
    pub program: Program,
    /// The raw source text of the entry file and of every distinct
    /// external module file actually loaded, in traversal order. `tint
    /// build` concatenates these to get one self-contained source blob it
    /// can embed -- `mod name;`/`use a::b;` lines are harmless no-ops to
    /// every downstream consumer (nothing walks `Item::Mod`/`Item::Use`),
    /// so plain concatenation is enough: no unparser, no textual rewriting.
    pub all_sources: Vec<String>,
}

/// Loads `entry_path`, resolves every `mod` it (transitively) declares
/// into real files, resolves every `use` against the exported items of the
/// module it names, and returns one flat `Program` -- see `Loaded`. Prints
/// a `module error: ...` line and exits the process on any failure (a
/// missing file, an unresolved `use`, importing a non-exported item, two
/// reachable items sharing a bare name, a circular `mod` chain) -- the
/// same "report and exit" convention `read_file`/`parse_source`'s callers
/// already use everywhere else in this CLI. `try_load` underneath is the
/// pure, `Result`-returning version this only wraps, kept separate so the
/// resolution logic itself is unit-testable without tearing down the test
/// process on the failure-path tests.
///
/// This is a preprocessing step done once, in the CLI, before anything
/// module-unaware (the checker, the IR compiler, `TintVM`) ever sees the
/// program -- mirroring how `ReplSession` flattens accumulated source into
/// one re-parsed `Program` rather than teaching the evaluator incremental
/// compilation. None of those consumers gain any notion of modules; they
/// just see a bigger single-file-shaped `Program`, as always.
///
/// Basic-version scope (matching this project's usual "narrow but real"
/// convention -- see the semantic checker's existence-only design): a
/// `use` path is always absolute from the entry file's own module tree (no
/// `self::`/`super::`). Grouped imports (`use a::{b, c};`, arbitrarily
/// nested), `as` aliasing (`use a::b as c;`), and wildcard imports
/// (`use a::*;`) ARE supported -- see `tint-parser`'s `parse_use_tail` for
/// the grammar and `resolve_use`/`resolve_use_wildcard` below for how each
/// resolves. Only `fn`/`struct`/`enum` can be exported/imported -- `ui fn`,
/// `impl`, `let`, kernels and type aliases stay local to the file that
/// declares them for now (matching what `export` already syntactically
/// applies to, see `parse_export.rs`).
/// Two reachable items ending up with the same bare name is a hard load
/// error: there's no per-module runtime scoping to fall back on for
/// disambiguation -- the VM's `logic_functions` table is one flat, global
/// map, exactly as it is for a single-file program today.
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
    let entry_source = read_file(entry_path);
    let program = match parse_source(&entry_source) {
        Ok(program) => program,
        Err(error) => {
            report_parse_error(&entry_source, &error);
            std::process::exit(1);
        }
    };

    let entry_dir = Path::new(entry_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let globals = program.globals.clone();

    let mut tree: ModuleTree = HashMap::new();
    let mut loading: HashSet<PathBuf> = HashSet::new();
    if let Ok(canon) = Path::new(entry_path).canonicalize() {
        loading.insert(canon);
    }
    let mut all_sources = vec![entry_source.clone()];
    collect_modules(
        &entry_dir,
        Vec::new(),
        program.items,
        &mut tree,
        &mut loading,
        &mut all_sources,
    )?;

    let items = resolve(&tree)?;

    Ok(Loaded {
        entry_source,
        program: Program { globals, items },
        all_sources,
    })
}

fn collect_modules(
    dir: &Path,
    path: Vec<String>,
    items: Vec<Item>,
    tree: &mut ModuleTree,
    loading: &mut HashSet<PathBuf>,
    all_sources: &mut Vec<String>,
) -> Result<(), String> {
    let mut own_items = Vec::new();

    for item in items {
        match item {
            Item::Mod(m) => {
                let mut child_path = path.clone();
                child_path.push(m.name.clone());

                if m.external {
                    let (child_dir, child_file, child_source, child_items) =
                        load_external_module(dir, &path, &m)?;

                    let canon = child_file.canonicalize().unwrap_or(child_file);
                    if !loading.insert(canon.clone()) {
                        return Err(format!(
                            "circular `mod` reference through `{}` (`{}` is already being loaded)",
                            module_path_str(&child_path),
                            canon.display(),
                        ));
                    }
                    all_sources.push(child_source);
                    collect_modules(
                        &child_dir,
                        child_path,
                        child_items,
                        tree,
                        loading,
                        all_sources,
                    )?;
                    loading.remove(&canon);
                } else {
                    collect_modules(dir, child_path, m.items, tree, loading, all_sources)?;
                }
            }
            other => own_items.push(other),
        }
    }

    tree.insert(path, own_items);
    Ok(())
}

/// Resolves `mod name;` to a real file relative to `dir`: `name.tn` first
/// (a leaf module sitting beside its declarer), else `name/mod.tn` (a
/// directory module -- the `mod.tn`-as-`mod.rs` convention). Returns the
/// directory a *nested* `mod` inside that file should itself resolve
/// against, the resolved file's own path (for cycle detection), its raw
/// source text, and its parsed items.
fn load_external_module(
    dir: &Path,
    parent_path: &[String],
    m: &ModDecl,
) -> Result<(PathBuf, PathBuf, String, Vec<Item>), String> {
    let flat_candidate = dir.join(format!("{}.tn", m.name));
    let dir_candidate = dir.join(&m.name).join("mod.tn");

    let (file_path, child_dir) = if flat_candidate.is_file() {
        (flat_candidate, dir.to_path_buf())
    } else if dir_candidate.is_file() {
        (dir_candidate, dir.join(&m.name))
    } else {
        let mut full_path = parent_path.to_vec();
        full_path.push(m.name.clone());
        return Err(format!(
            "cannot find module `{}` -- looked for `{}` and `{}`",
            module_path_str(&full_path),
            flat_candidate.display(),
            dir_candidate.display(),
        ));
    };

    let source = read_file(file_path.to_string_lossy().as_ref());
    let program = match parse_source(&source) {
        Ok(program) => program,
        Err(error) => {
            report_parse_error(&source, &error);
            std::process::exit(1);
        }
    };

    Ok((child_dir, file_path, source, program.items))
}

/// The bare name and export status of an item that can participate in
/// `use`/`export` -- `fn`/`struct`/`enum` only, matching `parse_export.rs`.
/// Everything else (`ui fn`, `impl`, `let`, kernels, type aliases, ...)
/// returns `None` and is never importable across a module boundary.
fn export_info(item: &Item) -> Option<(&str, bool)> {
    match item {
        Item::Fn(f) => Some((f.name.as_str(), false)),
        Item::ExportFn(f, _) => Some((f.name.as_str(), true)),
        Item::Struct(s) => Some((s.name.as_str(), false)),
        Item::ExportStruct(s) => Some((s.name.as_str(), true)),
        Item::Enum(e) => Some((e.name.as_str(), false)),
        Item::ExportEnum(e) => Some((e.name.as_str(), true)),
        _ => None,
    }
}

/// Renames an exportable item's own embedded name (`FnDecl`/`StructDecl`/
/// `EnumDecl.name`) to `new_name`. Needed for `use a::b as c;`: keying
/// `merged`/`order` under the alias `c` is not enough on its own, because
/// every other consumer (`item_names` in this file's own tests,
/// `tint-semantics`'s checker, `register_functions`, the IR compiler, ...)
/// reads a function's callable name straight off its own `FnDecl.name`,
/// never off whatever key `resolve()` happened to file it under. Without
/// this, an aliased import would be reachable under its ORIGINAL name once
/// flattened (since the clone's embedded name never changed), not the
/// alias the `.tn` source actually wrote.
fn rename_item(item: Item, new_name: &str) -> Item {
    match item {
        Item::Fn(mut f) => {
            f.name = new_name.to_string();
            Item::Fn(f)
        }
        Item::ExportFn(mut f, span) => {
            f.name = new_name.to_string();
            Item::ExportFn(f, span)
        }
        Item::Struct(mut s) => {
            s.name = new_name.to_string();
            Item::Struct(s)
        }
        Item::ExportStruct(mut s) => {
            s.name = new_name.to_string();
            Item::ExportStruct(s)
        }
        Item::Enum(mut e) => {
            e.name = new_name.to_string();
            Item::Enum(e)
        }
        Item::ExportEnum(mut e) => {
            e.name = new_name.to_string();
            Item::ExportEnum(e)
        }
        // Anything else never reaches here: `resolve_use`/`resolve_use_wildcard`
        // only ever call this on an item that already passed `export_info`'s
        // filter, which covers exactly the six arms above.
        other => other,
    }
}

fn module_path_str(path: &[String]) -> String {
    if path.is_empty() {
        "<entry file>".to_string()
    } else {
        path.join("::")
    }
}

/// Flattens `tree` into the entry file's own items plus everything actually
/// reachable through a `use` chain, starting from the root module and
/// following any module a resolved `use` pulls in (its own `use`s need
/// resolving too, since its body may reference names it imported).
fn resolve(tree: &ModuleTree) -> Result<Vec<Item>, String> {
    let root_items = tree.get(&Vec::<String>::new()).cloned().unwrap_or_default();

    // name -> (module path it actually came from, the item itself)
    let mut merged: HashMap<String, (Vec<String>, Item)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    // Root's own exportable items are always present, unconditionally --
    // this is exactly today's single-file behavior, `export` or not.
    for item in &root_items {
        if let Some((name, _)) = export_info(item) {
            insert_merged(
                &mut merged,
                &mut order,
                name.to_string(),
                Vec::new(),
                item.clone(),
            )?;
        }
    }

    let mut visited: HashSet<Vec<String>> = HashSet::new();
    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(Vec::new());

    while let Some(mod_path) = queue.pop_front() {
        if !visited.insert(mod_path.clone()) {
            continue;
        }
        let Some(items) = tree.get(&mod_path) else {
            continue;
        };
        for item in items {
            if let Item::Use(u) = item {
                resolve_use(tree, &mod_path, u, &mut merged, &mut order, &mut queue)?;
            }
        }
    }

    // Root's own items, in their original order, minus the `use` lines
    // themselves (each resolved `use` is emitted below instead) and minus
    // exportable items (already captured above, in `merged`, so a
    // `use`-imported name that collides with one of them is still caught).
    let mut output: Vec<Item> = Vec::new();
    for item in &root_items {
        match item {
            Item::Use(_) => {}
            _ if export_info(item).is_some() => {}
            _ => output.push(item.clone()),
        }
    }
    for name in &order {
        let (_origin, item) = &merged[name];
        output.push(item.clone());
    }

    Ok(output)
}

fn insert_merged(
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    name: String,
    origin: Vec<String>,
    item: Item,
) -> Result<(), String> {
    match merged.get(&name) {
        Some((existing_origin, _)) if *existing_origin == origin => {
            // The same item, reached twice (e.g. two modules both `use`
            // the same third module's function) -- not a real collision.
            Ok(())
        }
        Some((existing_origin, _)) => Err(format!(
            "`{}` is defined in both `{}` and `{}` -- this basic module system has no per-name \
             disambiguation yet, rename one of them",
            name,
            module_path_str(existing_origin),
            module_path_str(&origin),
        )),
        None => {
            order.push(name.clone());
            merged.insert(name, (origin, item));
            Ok(())
        }
    }
}

/// A `use` line's own text, reconstructed for error messages -- `u.path`
/// alone doesn't distinguish `use a::b;` from `use a::*;` (wildcard) or
/// `use a::b as c;` (aliased), so error messages built straight from
/// `u.path.join("::")` used to read fine for the plain case and silently
/// drop the `*`/`as c` the user actually wrote for the other two.
fn use_decl_str(u: &UseDecl) -> String {
    let base = u.path.join("::");
    if u.wildcard {
        format!("{}::*", base)
    } else if let Some(alias) = &u.alias {
        format!("{} as {}", base, alias)
    } else {
        base
    }
}

fn resolve_use(
    tree: &ModuleTree,
    importer_path: &[String],
    u: &UseDecl,
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    queue: &mut VecDeque<Vec<String>>,
) -> Result<(), String> {
    if u.wildcard {
        return resolve_use_wildcard(tree, importer_path, u, merged, order, queue);
    }

    if u.path.len() < 2 {
        return Err(format!(
            "`use {}` in `{}` -- a `use` path needs at least `module::item`",
            use_decl_str(u),
            module_path_str(importer_path),
        ));
    }

    let (mod_path, item_name) = u.path.split_at(u.path.len() - 1);
    let mod_path = mod_path.to_vec();
    let item_name = &item_name[0];
    // `use a::b as c;` binds the imported item under `c` in the merged,
    // flat namespace instead of its own bare name `b`; a plain `use a::b;`
    // (no alias) keeps today's behavior of binding it under `b`.
    let bound_name = u.alias.clone().unwrap_or_else(|| item_name.clone());

    let Some(target_items) = tree.get(&mod_path) else {
        return Err(format!(
            "`use {}` in `{}` -- no module `{}`",
            use_decl_str(u),
            module_path_str(importer_path),
            module_path_str(&mod_path),
        ));
    };

    let found = target_items.iter().find_map(|item| {
        export_info(item)
            .filter(|(name, _)| *name == item_name.as_str())
            .map(|(_, exported)| (item, exported))
    });

    match found {
        Some((item, true)) => {
            let item = match &u.alias {
                Some(alias) => rename_item(item.clone(), alias),
                None => item.clone(),
            };
            insert_merged(merged, order, bound_name, mod_path.clone(), item)?;
            queue.push_back(mod_path);
            Ok(())
        }
        Some((_, false)) => Err(format!(
            "`use {}` in `{}` -- `{}` exists in `{}` but isn't exported (add `export` before it)",
            use_decl_str(u),
            module_path_str(importer_path),
            item_name,
            module_path_str(&mod_path),
        )),
        None => Err(format!(
            "`use {}` in `{}` -- no exported item named `{}` in `{}`",
            use_decl_str(u),
            module_path_str(importer_path),
            item_name,
            module_path_str(&mod_path),
        )),
    }
}

/// `use a::*;` -- `u.path` names the MODULE directly (no trailing item
/// segment to split off, unlike the single-item case above). Every
/// exported item in it is imported under its own bare name; a name
/// collision with something already merged (from this or another import)
/// is caught by `insert_merged` exactly as it would be for an explicit
/// `use`, since a wildcard is just sugar for "import everything exported
/// here", not a separate resolution path.
fn resolve_use_wildcard(
    tree: &ModuleTree,
    importer_path: &[String],
    u: &UseDecl,
    merged: &mut HashMap<String, (Vec<String>, Item)>,
    order: &mut Vec<String>,
    queue: &mut VecDeque<Vec<String>>,
) -> Result<(), String> {
    let mod_path = u.path.clone();

    let Some(target_items) = tree.get(&mod_path) else {
        return Err(format!(
            "`use {}` in `{}` -- no module `{}`",
            use_decl_str(u),
            module_path_str(importer_path),
            module_path_str(&mod_path),
        ));
    };

    let exported: Vec<(String, Item)> = target_items
        .iter()
        .filter_map(|item| {
            export_info(item)
                .and_then(|(name, exported)| exported.then(|| (name.to_string(), item.clone())))
        })
        .collect();

    for (name, item) in exported {
        insert_merged(merged, order, name, mod_path.clone(), item)?;
    }
    queue.push_back(mod_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A scratch directory under `target/` (not `/tmp`, so it survives
    /// alongside the rest of the build and needs no extra cleanup logic
    /// beyond what a fresh `cargo clean` already does), unique per test run
    /// so parallel `cargo test` runs never see each other's fixture files.
    fn scratch_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("module_loader_test_scratch")
            .join(format!("{}-{}", tag, n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    fn item_names(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .filter_map(|i| export_info(i).map(|(name, _)| name.to_string()))
            .collect()
    }

    #[test]
    fn use_imports_an_exported_fn_and_drops_the_private_one() {
        let dir = scratch_dir("basic");
        write(
            &dir,
            "math.tn",
            "export fn add(a, b) { a + b }\nfn internal_helper() { 99 }\n",
        );
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add;\nfn main() { add(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("should load");
        let names = item_names(&loaded.program.items);
        assert!(
            names.contains(&"add".to_string()),
            "expected `add` to be pulled in: {:?}",
            names
        );
        assert!(names.contains(&"main".to_string()));
        assert!(
            !names.contains(&"internal_helper".to_string()),
            "private helper must not leak across the module boundary: {:?}",
            names
        );
        assert_eq!(loaded.all_sources.len(), 2, "entry + one submodule file");
    }

    #[test]
    fn directory_module_resolves_via_mod_tn() {
        let dir = scratch_dir("dirmod");
        write(&dir, "math/mod.tn", "export fn add(a, b) { a + b }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add;\nfn main() { add(1, 1) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("should load via math/mod.tn");
        assert!(item_names(&loaded.program.items).contains(&"add".to_string()));
    }

    #[test]
    fn nested_mod_resolves_relative_to_its_own_directory() {
        let dir = scratch_dir("nested");
        write(
            &dir,
            "math/mod.tn",
            "mod vec3;\nexport fn add(a, b) { a + b }\n",
        );
        write(&dir, "math/vec3.tn", "export fn len(v) { v }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add;\nfn main() { add(1, 1) }\n",
        );

        // `math`'s own `mod vec3;` isn't `use`d by anyone, so `len` stays
        // unreachable -- this just proves the nested file resolves (a typo
        // in its path would fail the whole load, not silently vanish).
        let loaded = try_load(main.to_str().unwrap()).expect("nested mod should resolve");
        assert!(item_names(&loaded.program.items).contains(&"add".to_string()));
    }

    #[test]
    fn importing_a_non_exported_item_is_an_error() {
        let dir = scratch_dir("private");
        write(&dir, "math.tn", "fn add(a, b) { a + b }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add;\nfn main() { add(1, 1) }\n",
        );

        let err = try_load(main.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("isn't exported"),
            "unexpected message: {}",
            err
        );
    }

    #[test]
    fn importing_a_missing_name_is_an_error() {
        let dir = scratch_dir("missing_name");
        write(&dir, "math.tn", "export fn add(a, b) { a + b }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::subtract;\nfn main() { 1 }\n",
        );

        let err = try_load(main.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("no exported item named"),
            "unexpected message: {}",
            err
        );
    }

    #[test]
    fn missing_module_file_is_an_error_naming_both_candidates() {
        let dir = scratch_dir("missing_file");
        let main = write(&dir, "main.tn", "mod ghost;\nfn main() { 1 }\n");

        let err = try_load(main.to_str().unwrap()).unwrap_err();
        assert!(err.contains("ghost.tn"), "{}", err);
        assert!(err.contains("ghost/mod.tn"), "{}", err);
    }

    #[test]
    fn colliding_names_from_two_modules_is_an_error() {
        let dir = scratch_dir("collision");
        write(&dir, "a.tn", "export fn thing() { 1 }\n");
        write(&dir, "b.tn", "export fn thing() { 2 }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod a;\nmod b;\nuse a::thing;\nuse b::thing;\nfn main() { thing() }\n",
        );

        let err = try_load(main.to_str().unwrap()).unwrap_err();
        assert!(err.contains("is defined in both"), "{}", err);
    }

    #[test]
    fn diamond_import_of_the_same_item_is_not_a_collision() {
        // `a` and `b` both re-export nothing themselves, but the entry
        // both directly `use`s the same underlying item -- fine, since a
        // repeated import of the identical origin is idempotent, not an
        // ambiguity.
        let dir = scratch_dir("diamond");
        write(&dir, "shared.tn", "export fn thing() { 1 }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod shared;\nuse shared::thing;\nuse shared::thing;\nfn main() { thing() }\n",
        );

        let loaded =
            try_load(main.to_str().unwrap()).expect("repeated identical use should be fine");
        assert!(item_names(&loaded.program.items).contains(&"thing".to_string()));
    }

    #[test]
    fn transitively_used_module_s_own_use_is_also_resolved() {
        // `helpers` isn't `use`d by the entry at all -- only `math` uses
        // it -- but since `add`'s body calls `helper()`, `helper` must
        // still end up in the final flat namespace once `math` becomes
        // reachable, or the program would be broken at runtime despite
        // loading "successfully".
        let dir = scratch_dir("transitive");
        write(&dir, "helpers.tn", "export fn helper() { 1 }\n");
        write(
            &dir,
            "math.tn",
            "use helpers::helper;\nexport fn add(a, b) { helper() }\n",
        );
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nmod helpers;\nuse math::add;\nfn main() { add(1, 1) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("transitive use should resolve");
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"add".to_string()));
        assert!(
            names.contains(&"helper".to_string()),
            "math's own `use helpers::helper;` must still be resolved once `add` is reachable: {:?}",
            names
        );
    }

    #[test]
    fn circular_mod_reference_is_an_error_not_a_stack_overflow() {
        let dir = scratch_dir("circular");
        write(&dir, "a.tn", "mod b;\nexport fn a_fn() { 1 }\n");
        write(&dir, "b.tn", "mod a;\nexport fn b_fn() { 1 }\n");
        let main = write(&dir, "main.tn", "mod a;\nfn main() { 1 }\n");

        let err = try_load(main.to_str().unwrap()).unwrap_err();
        assert!(err.contains("circular"), "{}", err);
    }

    #[test]
    fn imported_fn_actually_computes_through_the_real_vm_not_just_present_by_name() {
        // The other tests here only check that `add` ended up in
        // `program.items` under the right name -- this proves its BODY
        // actually runs and produces the right value once the flattened
        // program reaches a real `TintVM`, exactly the path `tint run`
        // takes.
        use tint_ast::Span;
        use tint_evaluator::value::Value;
        use tint_evaluator::EvalHost;
        use tint_runtime::vm::TintVM;

        let dir = scratch_dir("end_to_end");
        write(&dir, "math.tn", "export fn add(a, b) { a + b }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add;\nfn main() { add(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("should load");
        let mut vm = TintVM::new();
        vm.run_program(&loaded.program);

        match vm.call_fn(
            "add",
            &[Value::Number(2.0), Value::Number(3.0)],
            Span::dummy(),
        ) {
            Ok(Value::Number(n)) => assert_eq!(n, 5.0),
            other => panic!("expected Ok(Number(5)), got {:?}", other),
        }
    }

    #[test]
    fn a_program_with_no_modules_at_all_is_unchanged() {
        let dir = scratch_dir("nomod");
        let main = write(
            &dir,
            "main.tn",
            "fn main() { 1 }\nexport fn helper() { 2 }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("plain single-file program");
        assert_eq!(loaded.all_sources.len(), 1);
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"main".to_string()));
        assert!(names.contains(&"helper".to_string()));
    }

    #[test]
    fn use_group_imports_every_item_listed_from_one_statement() {
        let dir = scratch_dir("group");
        write(
            &dir,
            "math.tn",
            "export fn add(a, b) { a + b }\nexport fn sub(a, b) { a - b }\nfn internal_helper() { 99 }\n",
        );
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::{add, sub};\nfn main() { add(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("grouped use should resolve");
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"add".to_string()), "{:?}", names);
        assert!(names.contains(&"sub".to_string()), "{:?}", names);
        assert!(
            !names.contains(&"internal_helper".to_string()),
            "{:?}",
            names
        );
    }

    #[test]
    fn use_group_supports_a_trailing_comma_and_an_alias_together() {
        let dir = scratch_dir("group_alias");
        write(
            &dir,
            "math.tn",
            "export fn add(a, b) { a + b }\nexport fn sub(a, b) { a - b }\n",
        );
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::{add, sub as minus,};\nfn main() { add(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap())
            .expect("trailing comma + alias in a group should resolve");
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"add".to_string()), "{:?}", names);
        assert!(
            names.contains(&"minus".to_string()),
            "aliased group member should land under its alias: {:?}",
            names
        );
        assert!(
            !names.contains(&"sub".to_string()),
            "an aliased import should NOT also be reachable under its original name: {:?}",
            names
        );
    }

    #[test]
    fn use_alias_binds_the_imported_fn_under_the_local_name_and_it_actually_runs() {
        // Mirrors `imported_fn_actually_computes_through_the_real_vm_not_just_present_by_name`
        // above, but for the `as` path specifically: proves the alias is a
        // real rename all the way through to a callable `TintVM` function,
        // not just a name that happens to appear in `program.items`.
        use tint_ast::Span;
        use tint_evaluator::value::Value;
        use tint_evaluator::EvalHost;
        use tint_runtime::vm::TintVM;

        let dir = scratch_dir("alias_end_to_end");
        write(&dir, "math.tn", "export fn add(a, b) { a + b }\n");
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::add as plus;\nfn main() { plus(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("aliased use should resolve");
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"plus".to_string()), "{:?}", names);
        assert!(!names.contains(&"add".to_string()), "{:?}", names);

        let mut vm = TintVM::new();
        vm.run_program(&loaded.program);
        match vm.call_fn(
            "plus",
            &[Value::Number(2.0), Value::Number(3.0)],
            Span::dummy(),
        ) {
            Ok(Value::Number(n)) => assert_eq!(n, 5.0),
            other => panic!("expected Ok(Number(5)), got {:?}", other),
        }
    }

    #[test]
    fn use_wildcard_imports_every_exported_item_and_only_those() {
        let dir = scratch_dir("wildcard");
        write(
            &dir,
            "math.tn",
            "export fn add(a, b) { a + b }\nexport fn sub(a, b) { a - b }\nfn internal_helper() { 99 }\n",
        );
        let main = write(
            &dir,
            "main.tn",
            "mod math;\nuse math::*;\nfn main() { add(2, 3) }\n",
        );

        let loaded = try_load(main.to_str().unwrap()).expect("wildcard use should resolve");
        let names = item_names(&loaded.program.items);
        assert!(names.contains(&"add".to_string()), "{:?}", names);
        assert!(names.contains(&"sub".to_string()), "{:?}", names);
        assert!(
            !names.contains(&"internal_helper".to_string()),
            "a wildcard import must still respect `export` -- private items must not leak: {:?}",
            names
        );
    }

    #[test]
    fn use_wildcard_of_a_missing_module_is_a_clean_error() {
        let dir = scratch_dir("wildcard_missing");
        let main = write(&dir, "main.tn", "use nope::*;\nfn main() { 1 }\n");

        let err = try_load(main.to_str().unwrap()).expect_err("module doesn't exist");
        assert!(
            err.contains("no module `nope`"),
            "expected a clear missing-module error, got: {}",
            err
        );
    }
}
