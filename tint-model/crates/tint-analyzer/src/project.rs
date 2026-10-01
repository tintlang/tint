use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The entry file whose `import`s (transitively) pull `fragment` in, or `None`
/// when nothing imports it. Importers may sit in the same directory, a parent
/// directory or a sibling one (`site.tn` -> `landing/landing.tn` ->
/// `landing/topbar.tn`), so the whole project tree under the highest ancestor
/// directory that still holds `.tn` files is scanned. With several candidate
/// roots the one that reaches the most files wins.
pub(crate) fn find_project_entry(fragment: &Path) -> Option<PathBuf> {
    let target = fragment.canonicalize().ok()?;
    let mut top = target.parent()?.to_path_buf();
    for _ in 0..3 {
        match top.parent() {
            Some(parent) if dir_has_tn(parent) => top = parent.to_path_buf(),
            _ => break,
        }
    }
    let mut files = Vec::new();
    collect_tn_files(&top, 0, &mut files);
    let graph: HashMap<PathBuf, Vec<PathBuf>> = files
        .iter()
        .map(|file| {
            let text = fs::read_to_string(file).unwrap_or_default();
            (file.clone(), imports_of(file, &text))
        })
        .collect();
    let imported: std::collections::HashSet<&PathBuf> = graph.values().flatten().collect();
    if !imported.contains(&target) {
        return None;
    }
    let closure = |root: &PathBuf| {
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![root.clone()];
        while let Some(file) = stack.pop() {
            if seen.insert(file.clone()) {
                stack.extend(graph.get(&file).cloned().unwrap_or_default());
            }
        }
        seen
    };
    graph
        .keys()
        .filter(|file| !imported.contains(file))
        .map(|root| (root, closure(root)))
        .filter(|(_, reached)| reached.contains(&target))
        .max_by(|(a, x), (b, y)| x.len().cmp(&y.len()).then_with(|| b.cmp(a)))
        .map(|(root, _)| root.clone())
}

fn dir_has_tn(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("tn"))
        })
        .unwrap_or(false)
}

fn collect_tn_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                collect_tn_files(&path, depth + 1, out);
            }
        } else if path.extension().and_then(|x| x.to_str()) == Some("tn") {
            if let Ok(canonical) = path.canonicalize() {
                out.push(canonical);
            }
        }
    }
}

/// Files named by `import "..."` lines of `source` (existing ones only).
fn imports_of(importer: &Path, source: &str) -> Vec<PathBuf> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("import \"")?;
            let end = rest.find('"')?;
            let tail = rest[end + 1..].trim();
            if !tail.is_empty() && tail != ";" {
                return None;
            }
            importer.parent()?.join(&rest[..end]).canonicalize().ok()
        })
        .collect()
}
