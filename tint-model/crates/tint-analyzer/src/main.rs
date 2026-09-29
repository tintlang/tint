use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tint_compiler::resolver::resolve_path_with_overlays;
use tint_compiler::{expand_source_imports, parse_source_for_analyzer};
use tint_parser::error::ParserError;
use tint_semantics::{SemanticError, SemanticModel, Type};

#[derive(Default)]
struct Analyzer {
    documents: HashMap<String, String>,
}

struct CheckedDocument {
    source: String,
    model: SemanticModel,
    errors: Vec<SemanticError>,
}

impl Analyzer {
    fn check(&self, uri: &str) -> Option<CheckedDocument> {
        let source = self.documents.get(uri)?.clone();

        // Use the project resolver for `.tn` files so `mod`/`use` and sibling
        // modules participate in checking. Open documents are supplied as an
        // overlay, so diagnostics follow unsaved editor changes too.
        if let Some(path) = uri_to_path(uri) {
            let overlays = self
                .documents
                .iter()
                .filter_map(|(uri, text)| {
                    let path = uri_to_path(uri)?;
                    Some((path.canonicalize().unwrap_or(path), text.clone()))
                })
                .collect::<HashMap<_, _>>();
            if path.exists() {
                if let Ok(resolved) = resolve_path_with_overlays(&path, &overlays) {
                    let (errors, model) = resolved.check();
                    return Some(CheckedDocument {
                        source,
                        model,
                        errors,
                    });
                }

                // An imported UI fragment (`topbar.tn`, `hero.tn`, …) is
                // valid source but is not a standalone Tint program. Find
                // its entry file and use that file's declarations as the
                // semantic context for the fragment itself.
                if let Some(entry) = find_project_entry(&path) {
                    if let Ok(resolved) = resolve_path_with_overlays(&entry, &overlays) {
                        let program = parse_source_for_analyzer(&source).ok()?;
                        let context = resolved.context();
                        let mut checker = tint_semantics::SemanticChecker::new(Default::default());
                        let (errors, model) = checker.check_with_context(&program, &context);
                        return Some(CheckedDocument {
                            source,
                            model,
                            errors,
                        });
                    }
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
        })
    }

    fn diagnostics(&self, uri: &str) -> Value {
        let Some(source) = self.documents.get(uri) else {
            return json!([]);
        };
        let parsed = self
            .expanded_source(uri)
            .as_deref()
            .unwrap_or(source)
            .to_string();
        if let Err(error) = parse_source_for_analyzer(&parsed) {
            return json!([parse_diagnostic(&error)]);
        }
        let Some(document) = self.check(uri) else {
            return json!([]);
        };
        document
            .errors
            .iter()
            .map(|error| diagnostic(error, &document.source))
            .collect()
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
        let offset = position_offset(&document.source, position);
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
        let offset = position_offset(&document.source, position);
        let Some(reference) = document.model.references.iter().find(|reference| {
            reference.span.start.offset <= offset && offset <= reference.span.end.offset
        }) else {
            return Value::Null;
        };
        let Some(symbol) = document.model.symbol_named(&reference.name) else {
            return Value::Null;
        };
        json!({
            "uri": uri,
            "range": span_range(&symbol.span)
        })
    }
}

fn find_project_entry(fragment: &Path) -> Option<PathBuf> {
    let target = fragment.canonicalize().ok()?;
    let mut candidate = target.clone();
    let mut visited = std::collections::HashSet::new();
    let mut found_importer = false;

    while let Some(parent) = candidate.parent() {
        let parent = parent.to_path_buf();
        if !visited.insert(parent.clone()) {
            break;
        }
        let mut importer = None;
        for entry in fs::read_dir(&parent).ok()? {
            let file = entry.ok()?.path();
            if file.extension().and_then(|ext| ext.to_str()) != Some("tn") {
                continue;
            }
            let text = fs::read_to_string(&file).ok()?;
            if imports_file(&file, &text, &target) {
                importer = Some(file);
                break;
            }
        }
        if let Some(file) = importer {
            candidate = file;
            found_importer = true;
        } else {
            return found_importer.then_some(candidate);
        }
    }
    Some(candidate)
}

fn imports_file(importer: &Path, source: &str, target: &Path) -> bool {
    source.lines().any(|line| {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("import \"") else {
            return false;
        };
        let Some(end) = rest.find('"') else {
            return false;
        };
        let tail = rest[end + 1..].trim();
        if !tail.is_empty() && tail != ";" {
            return false;
        }
        importer
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&rest[..end])
            .canonicalize()
            .map(|path| path == target)
            .unwrap_or(false)
    })
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

fn parse_diagnostic(error: &ParserError) -> Value {
    let (span, message) = match error {
        ParserError::Message { msg, span } => (*span, friendly_message(msg)),
        ParserError::Unexpected {
            expected,
            found,
            span,
        } => (*span, friendly_unexpected(expected.clone(), found.clone())),
    };
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
