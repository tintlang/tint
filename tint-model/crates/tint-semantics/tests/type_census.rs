//! Reports how many inferred expression types are still `Unknown`.
//! Run: `cargo test -p tint-semantics --test type_census -- --nocapture --ignored`
//! Extra sources can be passed with TINT_CENSUS_FILES (colon separated).

use std::collections::HashMap;
use std::path::Path;

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::{SemanticChecker, Type};

fn sources() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = Vec::new();
    for dir in ["conformance", "examples"] {
        let mut paths = Vec::new();
        collect(&root.join(dir), dir == "conformance", &mut paths);
        paths.sort();
        out.extend(paths.into_iter().map(|p| p.display().to_string()));
    }
    if let Ok(extra) = std::env::var("TINT_CENSUS_FILES") {
        out.extend(extra.split(':').filter(|s| !s.is_empty()).map(String::from));
    }
    out
}

fn collect(dir: &Path, recursive: bool, out: &mut Vec<std::path::PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for path in read.filter_map(|e| e.ok()).map(|e| e.path()) {
        if path.is_dir() {
            if recursive {
                collect(&path, recursive, out);
            }
        } else if path.extension().is_some_and(|e| e == "tn") {
            out.push(path);
        }
    }
}

#[test]
#[ignore]
fn census() {
    let mut total = 0usize;
    let mut unknown = 0usize;
    let mut snippets: HashMap<String, usize> = HashMap::new();
    for path in sources() {
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = collect_tokens(&mut Lexer::new(&source));
        let Ok(program) = Parser::new(tokens).parse_program() else {
            println!("skip (parse error): {path}");
            continue;
        };
        let (errors, model) = SemanticChecker::new(CheckerContext::default()).check_with_model(&program);
        let (mut t, mut u) = (0, 0);
        for typed in &model.expressions {
            t += 1;
            if contains_unknown(&typed.ty) {
                u += 1;
                let text = source
                    .get(typed.span.start.offset..typed.span.end.offset)
                    .unwrap_or("?")
                    .replace('\n', " ");
                let text: String = text.chars().take(48).collect();
                *snippets.entry(text).or_default() += 1;
            }
        }
        println!("{path}: {t} exprs, {u} unknown, {} errors", errors.len());
        if std::env::var("TINT_CENSUS_VERBOSE").is_ok() { for e in &errors { let l = source[..e.span.start.offset.min(source.len())].matches('\n').count()+1; println!("    ERR L{l}: {:?}", e.kind); } for typed in &model.expressions { if contains_unknown(&typed.ty) { let l = source[..typed.span.start.offset].matches('\n').count()+1; let text: String = source.get(typed.span.start.offset..typed.span.end.offset).unwrap_or("?").replace('\n', " ").chars().take(60).collect(); println!("    L{l}: {:?} <- {text}", typed.ty); } } }
        total += t;
        unknown += u;
    }
    println!("TOTAL {total} exprs, {unknown} with unknown ({:.1}%)", 100.0 * unknown as f64 / total.max(1) as f64);
    let mut list: Vec<_> = snippets.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (text, count) in list.into_iter().take(40) {
        println!("{count:5}  {text}");
    }
}

fn contains_unknown(ty: &Type) -> bool {
    match ty {
        Type::Unknown => true,
        Type::Array(t) | Type::Map(t) | Type::Tensor(t) => contains_unknown(t),
        Type::Tuple(items) => items.iter().any(contains_unknown),
        Type::Generic(_, args) => args.iter().any(contains_unknown),
        Type::Fn(ret, params) => contains_unknown(ret) || params.iter().any(contains_unknown),
        _ => false,
    }
}

/// Stage 0 done-criterion: on the conformance suite and the examples no
/// expression is left `Unknown`, and strict mode has nothing to complain about.
#[test]
fn conformance_and_examples_are_fully_typed() {
    let mut checked = 0;
    for path in sources() {
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = collect_tokens(&mut Lexer::new(&source));
        let Ok(program) = Parser::new(tokens).parse_program() else { continue };
        let ctx = CheckerContext { strict: true, ..Default::default() };
        let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
        let open: Vec<_> = errors
            .iter()
            .filter(|e| matches!(e.kind, tint_semantics::errors::SemanticErrorKind::CannotInfer(_)))
            .collect();
        assert!(open.is_empty(), "{path}: could not infer: {open:?}");
        assert!(model.is_fully_typed(), "{path}: expressions left unknown");
        assert!(
            model.expressions.iter().all(|typed| !contains_unknown(&typed.ty)),
            "{path}: expressions left unknown"
        );
        checked += 1;
    }
    assert!(checked > 20, "only {checked} files were checked");
}
