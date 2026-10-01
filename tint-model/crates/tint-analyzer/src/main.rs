use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tint_compiler::resolver::resolve_path_with_overlays;
use tint_compiler::{expand_source_imports, expand_source_imports_origins, parse_source_for_analyzer};
use tint_parser::error::ParserError;
use tint_semantics::{SemanticError, SemanticModel, Type};

#[derive(Default)]
struct Analyzer {
    documents: HashMap<String, String>,
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
        self.origins.iter().position(|(path, index)| path == &self.target && *index == line)
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
                    return Some(CheckedDocument { source, model, errors, view: None });
                }
            }
        }

        let program = parse_source_for_analyzer(&source).ok()?;
        let (errors, model) =
            tint_semantics::SemanticChecker::new(Default::default()).check_with_model(&program);
        Some(CheckedDocument { source, model, errors, view: None })
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
        Some(CheckedDocument { source: expanded, model, errors, view: Some(view) })
    }

    fn diagnostics(&self, uri: &str) -> Value {
        let Some(source) = self.documents.get(uri) else {
            return json!([]);
        };
        let document = self.check(uri);
        // Inside a project the whole program was parsed already; a lone fragment
        // (`demo.tn`) is not a program by itself, so only parse it alone otherwise.
        if document.as_ref().map_or(true, |document| document.view.is_none()) {
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
            span.end.line = origins.get(span.end.line.checked_sub(1)?).map_or(line + 1, |(_, l)| l + 1);
            return Some(json!({
                "range": span_range(&span), "severity": 1, "source": "tint parser", "message": message
            }));
        }
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
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

    fn hover(&self, uri: &str, position: &Value) -> Value {
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

    fn definition(&self, uri: &str, position: &Value) -> Value {
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
            symbol.span.start.line.checked_sub(1).and_then(|line| view.origins.get(line)),
            symbol.span.end.line.checked_sub(1).and_then(|line| view.origins.get(line)),
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

/// The entry file whose `import`s (transitively) pull `fragment` in, or `None`
/// when nothing imports it. Importers may sit in the same directory, a parent
/// directory or a sibling one (`site.tn` -> `landing/landing.tn` ->
/// `landing/topbar.tn`), so the whole project tree under the highest ancestor
/// directory that still holds `.tn` files is scanned. With several candidate
/// roots the one that reaches the most files wins.
fn find_project_entry(fragment: &Path) -> Option<PathBuf> {
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
    let Ok(entries) = fs::read_dir(dir) else { return };
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

fn main() {
    let stdin = io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let mut analyzer = Analyzer::default();
    let stdout = io::stdout();
    let mut output = stdout.lock();

    while let Some(message) = read_message(&mut input) {
        let Ok(request) = serde_json::from_slice::<Value>(&message) else {
            continue;
        };
        if let Some(response) = handle(&mut analyzer, &request) {
            write_message(&mut output, &response);
        }
    }
}

fn handle(analyzer: &mut Analyzer, request: &Value) -> Option<Value> {
    let method = request.get("method")?.as_str()?;
    let id = request.get("id").cloned();
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "capabilities": {
                        "textDocumentSync": 1,
                        "hoverProvider": true,
                        "definitionProvider": true
                    },
                    "serverInfo": { "name": "tint-analyzer", "version": env!("CARGO_PKG_VERSION") }
                }
            })
        }),
        "shutdown" => id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": null })),
        "textDocument/didOpen" => {
            let document = params.get("textDocument")?;
            let uri = document.get("uri")?.as_str()?.to_string();
            let text = document.get("text")?.as_str()?.to_string();
            analyzer.documents.insert(uri.clone(), text);
            Some(publish_diagnostics(analyzer, &uri))
        }
        "textDocument/didChange" => {
            let document = params.get("textDocument")?;
            let uri = document.get("uri")?.as_str()?.to_string();
            let text = params
                .get("contentChanges")?
                .as_array()?
                .first()?
                .get("text")?
                .as_str()?
                .to_string();
            analyzer.documents.insert(uri.clone(), text);
            Some(publish_diagnostics(analyzer, &uri))
        }
        "textDocument/hover" => {
            let document = params.get("textDocument")?;
            let uri = document.get("uri")?.as_str()?;
            let position = params.get("position")?;
            id.map(
                |id| json!({ "jsonrpc": "2.0", "id": id, "result": analyzer.hover(uri, position) }),
            )
        }
        "textDocument/definition" => {
            let document = params.get("textDocument")?;
            let uri = document.get("uri")?.as_str()?;
            let position = params.get("position")?;
            id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": analyzer.definition(uri, position) }))
        }
        _ => id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": null })),
    }
}

fn publish_diagnostics(analyzer: &Analyzer, uri: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": analyzer.diagnostics(uri) }
    })
}

fn diagnostic(error: &SemanticError, source: &str) -> Value {
    let message = format!("{:?}", error.kind);
    json!({
        "range": span_range(&error.span),
        "severity": 1,
        "source": "tint",
        "message": message,
        "data": { "sourceLength": source.len() }
    })
}

fn parse_error_parts(error: &ParserError) -> (tint_ast::Span, String) {
    match error {
        ParserError::Message { msg, span } => (*span, friendly_message(msg)),
        ParserError::Unexpected {
            expected,
            found,
            span,
        } => (*span, friendly_unexpected(expected.clone(), found.clone())),
    }
}

fn parse_diagnostic(error: &ParserError) -> Value {
    let (span, message) = parse_error_parts(error);
    json!({
        "range": span_range(&span),
        "severity": 1,
        "source": "tint parser",
        "message": message
    })
}

fn friendly_message(message: &str) -> String {
    if message.contains("Unexpected token Ident") && message.contains("expected fn/ui/struct/enum")
    {
        return "Unexpected UI node here. This file looks like an imported UI fragment; check for a missing `}` before this node if the error is real.".into();
    }
    message.into()
}

fn friendly_unexpected(expected: tint_lexer::TokenKind, found: tint_lexer::TokenKind) -> String {
    if expected == tint_lexer::TokenKind::RBrace && found == tint_lexer::TokenKind::Eof {
        return "Expected `}` before the end of the file. A UI block above is not closed.".into();
    }
    if found == tint_lexer::TokenKind::Ident
        && matches!(
            expected,
            tint_lexer::TokenKind::RBrace | tint_lexer::TokenKind::Eof
        )
    {
        return "Unexpected identifier here. A previous UI block may be missing its closing `}`."
            .into();
    }

    format!("Expected {:?}, but found {:?}.", expected, found)
}

fn span_range(span: &tint_ast::Span) -> Value {
    json!({
        "start": { "line": span.start.line.saturating_sub(1), "character": span.start.column.saturating_sub(1) },
        "end": { "line": span.end.line.saturating_sub(1), "character": span.end.column.saturating_sub(1) }
    })
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Number => "number".into(),
        Type::String => "string".into(),
        Type::Bool => "bool".into(),
        Type::Unit => "unit".into(),
        Type::Struct(name) | Type::Enum(name) | Type::Simple(name) => name.clone(),
        Type::Array(_) => "array".into(),
        Type::Tuple(_) => "tuple".into(),
        Type::Map(_) => "map".into(),
        Type::Fn(_, _) => "function".into(),
        Type::Unknown => "unknown".into(),
        other => format!("{:?}", other),
    }
}

fn position_offset(source: &str, position: &Value) -> usize {
    let line = position.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
    let character = position
        .get("character")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let mut offset = 0;
    for (index, text_line) in source.split_inclusive('\n').enumerate() {
        if index == line {
            return offset + character.min(text_line.trim_end_matches('\n').len());
        }
        offset += text_line.len();
    }
    source.len()
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    // VS Code emits file URIs with an absolute path. Keep this deliberately
    // dependency-free; percent-encoded paths are handled by the fallback
    // in-memory path until a URI crate is introduced for the full LSP layer.
    Some(PathBuf::from(path))
}

fn read_message(input: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut line = String::new();
    let mut length = None;
    loop {
        line.clear();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    Some(body)
}

fn write_message(output: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    write!(output, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    output.write_all(&body).unwrap();
    output.flush().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_are_checked_inside_their_project() {
        let root = std::env::temp_dir().join(format!("tint-analyzer-{}", std::process::id()));
        fs::create_dir_all(root.join("parts")).unwrap();
        fs::write(root.join("fns.tn"), "fn greet() { 1 }\n").unwrap();
        fs::write(
            root.join("main.tn"),
            "import \"./fns.tn\";\nui fn App() {\n    Column {\n        import \"./parts/bar.tn\";\n    }\n}\n",
        )
        .unwrap();
        let bar = root.join("parts/bar.tn");
        fs::write(&bar, "Text { click||greet \"hi\" }\n").unwrap();

        let uri = format!("file://{}", bar.display());
        let mut analyzer = Analyzer::default();
        analyzer.documents.insert(uri.clone(), fs::read_to_string(&bar).unwrap());
        assert_eq!(analyzer.diagnostics(&uri), json!([]));

        // A real error is still reported, on the fragment's own line.
        analyzer.documents.insert(uri.clone(), "\nText { click||nope \"hi\" }\n".into());
        let found = analyzer.diagnostics(&uri);
        assert_eq!(found[0]["range"]["start"]["line"], 1);
        fs::remove_dir_all(&root).ok();
    }
}
