use std::fs;

use tint_ast::{Item, Program};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_semantics::errors::SemanticError;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

pub(crate) fn require_arg<'a>(arg: Option<&'a String>, usage: &str) -> &'a str {
    arg.map(String::as_str).unwrap_or_else(|| {
        eprintln!("usage: {}", usage);
        std::process::exit(1);
    })
}

pub(crate) fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("error reading '{}': {}", path, error);
            std::process::exit(1);
        }
    }
}

pub(crate) fn parse_source(source: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    Parser::new(tokens).parse_program()
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

pub(crate) fn semantic_check(program: &Program) -> Vec<SemanticError> {
    SemanticChecker::new(CheckerContext::default()).check(program)
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
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(function) if function.name == name))
}

pub(crate) fn check_command(path: Option<&String>) {
    let path = require_arg(path, "tint check <file.tn>");
    let source = read_file(path);
    let program = match parse_source(&source) {
        Ok(program) => program,
        Err(error) => {
            report_parse_error(&source, &error);
            std::process::exit(1);
        }
    };

    let errors = semantic_check(&program);
    if !errors.is_empty() {
        for error in &errors {
            report_semantic_error(&source, error);
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
