use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tint_compiler::resolver::resolve_path_with_overlays;
use tint_compiler::{
    expand_source_imports, expand_source_imports_origins, parse_source_for_analyzer,
};
use tint_semantics::{SemanticError, SemanticModel};

use crate::project::find_project_entry;
use crate::protocol::{
    diagnostic, parse_diagnostic, parse_error_parts, position_offset, span_range, type_name,
    uri_to_path,
};

#[derive(Default)]
pub(crate) struct Analyzer {
    pub(crate) documents: HashMap<String, String>,
}

struct CheckedDocument {
    /// The text the model's offsets refer to: the open file, or -- for a file
    /// that is part of a bigger project -- the whole expanded project.
    source: String,
    model: SemanticModel,
    errors: Vec<SemanticError>,
    /// For a project check: the file each expanded line was written in, and
    /// the open file, so results can be mapped back onto it.
    view: Option<ProjectView>,
}

struct ProjectView {
    origins: Vec<(PathBuf, usize)>,
    target: PathBuf,
}

impl CheckedDocument {
    /// An LSP position in the open file, as a position in `source`.
    fn expanded_position(&self, position: &Value) -> Option<Value> {
        let Some(view) = &self.view else {
            return Some(position.clone());
        };
        let line = position.get("line")?.as_u64()? as usize;
        Some(json!({
            "line": view.to_expanded(line)?,
            "character": position.get("character").cloned().unwrap_or(json!(0))
        }))
    }
}

impl ProjectView {
    /// Expanded 0-based line -> line of the open file (`None` if it came from another file).
    fn to_file(&self, line: usize) -> Option<usize> {
        let (path, index) = self.origins.get(line)?;
        (path == &self.target).then_some(*index)
    }

    fn to_expanded(&self, line: usize) -> Option<usize> {
        self.origins
            .iter()
            .position(|(path, index)| path == &self.target && *index == line)
    }

    fn span(&self, span: &tint_ast::Span) -> Option<tint_ast::Span> {
        let mut span = *span;
        span.start.line = self.to_file(span.start.line.checked_sub(1)?)? + 1;
        span.end.line = self.to_file(span.end.line.checked_sub(1)?)? + 1;
        Some(span)
    }
}

impl Analyzer {
    fn check(&self, uri: &str) -> Option<CheckedDocument> {
        let source = self.documents.get(uri)?.clone();

        // Use the project resolver for `.tn` files so `mod`/`use` and sibling
        // modules participate in checking. Open documents are supplied as an
        // overlay, so diagnostics follow unsaved editor changes too.
        if let Some(path) = uri_to_path(uri) {
            let overlays = self.overlays();
            if path.exists() {
                if let Some(document) = self.check_in_project(&path, &source, &overlays) {
                    return Some(document);
                }
                if let Ok(resolved) = resolve_path_with_overlays(&path, &overlays) {
                    let (errors, model) = resolved.check();
                    return Some(CheckedDocument {
                        source,
                        model,
                        errors,
                        view: None,
                    });
                }
            }
        }

        let program = parse_source_for_analyzer(&source).ok()?;
        let (errors, model) =
            tint_semantics::SemanticChecker::new(Default::default()).check_with_model(&program);
        Some(CheckedDocument {
            source,
            model,
            errors,
            view: None,
        })
    }

    fn overlays(&self) -> HashMap<PathBuf, String> {
        self.documents
            .iter()
            .filter_map(|(uri, text)| {
                let path = uri_to_path(uri)?;
                Some((path.canonicalize().unwrap_or(path), text.clone()))
            })
            .collect()
    }

    /// Checks the whole project the file belongs to (its entry file, which for
    /// an imported fragment such as `topbar.tn` is the file that pulls it in),
    /// then keeps only what concerns this file. A fragment sees every fn, style,
    /// token and `state` of the `ui fn` it is spliced into, which a lone parse
    /// of the fragment cannot know.
    fn check_in_project(
        &self,
        path: &Path,
        source: &str,
        overlays: &HashMap<PathBuf, String>,
    ) -> Option<CheckedDocument> {
        let target = path.canonicalize().ok()?;
        let entry = find_project_entry(path).unwrap_or_else(|| target.clone());
        let resolved = resolve_path_with_overlays(&entry, overlays).ok()?;
        let (expanded, origins) = expand_source_imports_origins(&entry, overlays).ok()?;
        if expanded.lines().count() != origins.len()
            || resolved.entry_source.lines().count() != origins.len()
        {
            return None;
        }
        let (errors, model) = resolved.check();
        let view = ProjectView { origins, target };
        let errors = errors
            .into_iter()
            .filter_map(|mut error| {
                error.span = view.span(&error.span)?;
                Some(error)
            })
            .collect();
        let _ = source;
        Some(CheckedDocument {
            source: expanded,
            model,
            errors,
            view: Some(view),
        })
    }

    pub(crate) fn diagnostics(&self, uri: &str) -> Value {
        let Some(source) = self.documents.get(uri) else {
            return json!([]);
        };
        let document = self.check(uri);
        // Inside a project the whole program was parsed already; a lone fragment
        // (`demo.tn`) is not a program by itself, so only parse it alone otherwise.
        if document
            .as_ref()
            .map_or(true, |document| document.view.is_none())
        {
            if let Some(diagnostic) = self.project_parse_error(uri) {
                return json!([diagnostic]);
            }
            let parsed = self
                .expanded_source(uri)
                .as_deref()
                .unwrap_or(source)
                .to_string();
            if let Err(error) = parse_source_for_analyzer(&parsed) {
                return json!([parse_diagnostic(&error)]);
            }
        }
        let Some(document) = document else {
            return json!([]);
        };
        let mut seen = std::collections::HashSet::new();
        document
            .errors
            .iter()
            .map(|error| diagnostic(error, &document.source))
            .filter(|diagnostic| seen.insert(diagnostic.to_string()))
            .collect()
    }

    /// A syntax error anywhere in the project this file belongs to, placed on
    /// the right line of this file, or noted on line 1 when it is in another file.
    fn project_parse_error(&self, uri: &str) -> Option<Value> {
        let path = uri_to_path(uri)?;
        let target = path.canonicalize().ok()?;
        let entry = find_project_entry(&path).unwrap_or_else(|| target.clone());
        let (text, origins) = expand_source_imports_origins(&entry, &self.overlays()).ok()?;
        let error = parse_source_for_analyzer(&text).err()?;
        let (mut span, message) = parse_error_parts(&error);
        let (file, line) = origins.get(span.start.line.checked_sub(1)?)?;
        if file == &target {
            span.start.line = line + 1;
            span.end.line = origins
                .get(span.end.line.checked_sub(1)?)
                .map_or(line + 1, |(_, l)| l + 1);
            return Some(json!({
                "range": span_range(&span), "severity": 1, "source": "tint parser", "message": message
            }));
        }
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some(json!({
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 1 } },
            "severity": 2, "source": "tint parser",
            "message": format!("{} (in {}:{})", message, name, line + 1)
        }))
    }

    fn expanded_source(&self, uri: &str) -> Option<String> {
        let path = uri_to_path(uri)?;
        if !path.exists() {
            return None;
        }
        let overlays = self
            .documents
            .iter()
            .filter_map(|(uri, text)| {
                let path = uri_to_path(uri)?;
                Some((path.canonicalize().unwrap_or(path), text.clone()))
            })
            .collect::<HashMap<_, _>>();
        expand_source_imports(path, &overlays).ok()
    }

    pub(crate) fn hover(&self, uri: &str, position: &Value) -> Value {
        let Some(document) = self.check(uri) else {
            return Value::Null;
        };
        let Some(position) = document.expanded_position(position) else {
            return Value::Null;
        };
        let offset = position_offset(&document.source, &position);
        let Some(ty) = document.model.type_at(offset) else {
            return Value::Null;
        };
        json!({
            "contents": {
                "kind": "markdown",
                "value": format!("```tint\n{}\n```", type_name(ty))
            }
        })
    }

    pub(crate) fn definition(&self, uri: &str, position: &Value) -> Value {
        let Some(document) = self.check(uri) else {
            return Value::Null;
        };
        let Some(position) = document.expanded_position(position) else {
            return Value::Null;
        };
        let offset = position_offset(&document.source, &position);
        let Some(reference) = document.model.references.iter().find(|reference| {
            reference.span.start.offset <= offset && offset <= reference.span.end.offset
        }) else {
            return Value::Null;
        };
        let Some(symbol) = document.model.symbol_named(&reference.name) else {
            return Value::Null;
        };
        let Some(view) = &document.view else {
            return json!({ "uri": uri, "range": span_range(&symbol.span) });
        };
        // The symbol may live in another file of the project.
        let (Some((file, start)), Some((_, end))) = (
            symbol
                .span
                .start
                .line
                .checked_sub(1)
                .and_then(|line| view.origins.get(line)),
            symbol
                .span
                .end
                .line
                .checked_sub(1)
                .and_then(|line| view.origins.get(line)),
        ) else {
            return Value::Null;
        };
        json!({
            "uri": format!("file://{}", file.display()),
            "range": {
                "start": { "line": start, "character": symbol.span.start.column.saturating_sub(1) },
                "end": { "line": end, "character": symbol.span.end.column.saturating_sub(1) }
            }
        })
    }
}
