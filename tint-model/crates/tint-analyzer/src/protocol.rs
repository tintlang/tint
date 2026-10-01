use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};
use tint_parser::error::ParserError;
use tint_semantics::{SemanticError, Type};

use crate::analyzer::Analyzer;

pub(crate) fn handle(analyzer: &mut Analyzer, request: &Value) -> Option<Value> {
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

pub(crate) fn diagnostic(error: &SemanticError, source: &str) -> Value {
    let message = format!("{:?}", error.kind);
    json!({
        "range": span_range(&error.span),
        "severity": 1,
        "source": "tint",
        "message": message,
        "data": { "sourceLength": source.len() }
    })
}

pub(crate) fn parse_error_parts(error: &ParserError) -> (tint_ast::Span, String) {
    match error {
        ParserError::Message { msg, span } => (*span, friendly_message(msg)),
        ParserError::Unexpected {
            expected,
            found,
            span,
        } => (*span, friendly_unexpected(expected.clone(), found.clone())),
    }
}

pub(crate) fn parse_diagnostic(error: &ParserError) -> Value {
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

pub(crate) fn span_range(span: &tint_ast::Span) -> Value {
    json!({
        "start": { "line": span.start.line.saturating_sub(1), "character": span.start.column.saturating_sub(1) },
        "end": { "line": span.end.line.saturating_sub(1), "character": span.end.column.saturating_sub(1) }
    })
}

pub(crate) fn type_name(ty: &Type) -> String {
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

pub(crate) fn position_offset(source: &str, position: &Value) -> usize {
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

pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    // VS Code emits file URIs with an absolute path. Keep this deliberately
    // dependency-free; percent-encoded paths are handled by the fallback
    // in-memory path until a URI crate is introduced for the full LSP layer.
    Some(PathBuf::from(path))
}

pub(crate) fn read_message(input: &mut impl BufRead) -> Option<Vec<u8>> {
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

pub(crate) fn write_message(output: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    write!(output, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    output.write_all(&body).unwrap();
    output.flush().unwrap();
}
