use crate::module_loader::Loaded;
use tint_ast::{Item, Program};
use tint_parser::error::ParserError;
use tint_semantics::errors::SemanticError;

pub(crate) fn require_arg<'a>(arg: Option<&'a String>, usage: &str) -> &'a str {
    arg.map(String::as_str).unwrap_or_else(|| {
        eprintln!("usage: {}", usage);
        std::process::exit(1);
    })
}

pub(crate) fn parse_source(source: &str) -> Result<Program, ParserError> {
    tint_compiler::parse_source(source)
}

pub(crate) fn report_parse_error(source: &str, error: &ParserError) {
    eprintln!("parse error: {:?}", error);

    let span = match error {
        ParserError::Message { span, .. } | ParserError::Unexpected { span, .. } => *span,
    };
    let start = span.start.offset.min(source.len());
    let end = span.end.offset.min(source.len());
    eprintln!(
        "  at {}:{} -> `{}`",
        span.start.line,
        span.start.column,
        &source[start..end]
    );
}

pub(crate) fn semantic_check(loaded: &Loaded) -> Vec<SemanticError> {
    loaded.resolved.check().0
}

pub(crate) fn report_semantic_error(source: &str, error: &SemanticError) {
    let start = error.span.start.offset.min(source.len());
    let end = error.span.end.offset.min(source.len());
    eprintln!("semantic error: {:?}", error.kind);
    eprintln!(
        "  at {}:{} -> `{}`",
        error.span.start.line,
        error.span.start.column,
        &source[start..end]
    );
}

pub(crate) fn has_fn(program: &Program, name: &str) -> bool {
    program.items.iter().any(|item| match item {
        Item::Fn(function) | Item::ExportFn(function, _) => function.name == name,
        _ => false,
    })
}

pub(crate) fn check_command(path: Option<&String>) {
    let path = require_arg(path, "tint check <file.tn>");
    let loaded = crate::module_loader::load(path);
    let program = &loaded.program;

    let errors = semantic_check(&loaded);
    if !errors.is_empty() {
        for error in &errors {
            report_semantic_error(&loaded.entry_source, error);
        }
        eprintln!(
            "{} semantic error(s) ({} top-level item(s) parsed)",
            errors.len(),
            program.items.len()
        );
        std::process::exit(1);
    }

    println!(
        "OK: {} top-level item(s) parsed, semantic check passed",
        program.items.len()
    );
}
