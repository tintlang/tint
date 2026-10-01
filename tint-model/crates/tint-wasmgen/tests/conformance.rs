//! Differential test: every conformance case the wasm backend can compile
//! must give what the interpreter is expected to give (run under wasmi).
//! Cases it cannot compile yet are counted, not failed.

use std::path::{Path, PathBuf};
mod common;
use common::run;
use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

#[derive(Debug)]
enum Expect {
    Value(String),
    Error(String),
}

struct Case {
    file: PathBuf,
    entry: String,
    expect: Expect,
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "tn") {
            out.push(path);
        }
    }
}

fn cases_of(path: &Path) -> Vec<Case> {
    let source = std::fs::read_to_string(path).unwrap();
    let mut cases = Vec::new();
    let (mut pending, mut ir, mut xfail): (Option<Expect>, Option<Expect>, bool) = (None, None, false);
    for line in source.lines() {
        let t = line.trim();
        if let Some(r) = t.strip_prefix("// expect-error:") {
            pending = Some(Expect::Error(r.trim().into()));
        } else if let Some(r) = t.strip_prefix("// expect:") {
            pending = Some(Expect::Value(r.trim().into()));
        } else if let Some(r) = t.strip_prefix("// ir-expect-error:") {
            ir = Some(Expect::Error(r.trim().into()));
        } else if let Some(r) = t.strip_prefix("// ir-expect:") {
            ir = Some(Expect::Value(r.trim().into()));
        } else if t.starts_with("// xfail:") {
            xfail = true;
        } else if let Some(rest) = t.strip_prefix("fn ") {
            if let Some(expect) = pending.take() {
                let entry: String =
                    rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if !xfail {
                    cases.push(Case { file: path.to_path_buf(), entry, expect: ir.take().unwrap_or(expect) });
                }
            }
            (ir, xfail) = (None, false);
        }
    }
    cases
}

#[test]
fn wasm_backend_agrees_with_the_interpreter() {
    let mut files = Vec::new();
    collect(&conformance_dir(), &mut files);
    files.sort();
    let (mut ok, mut unsupported, mut failures) = (0, 0, Vec::new());
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    for file in &files {
        let source = std::fs::read_to_string(file).unwrap();
        let tokens = collect_tokens(&mut Lexer::new(&source));
        let Ok(program) = Parser::new(tokens).parse_program() else { continue };
        let ctx = CheckerContext { strict: true, ..Default::default() };
        let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
        if !errors.is_empty() {
            continue;
        }
        let lowered = lower_program(&program, &model);
        if !lowered.errors.is_empty() {
            continue;
        }
        for case in cases_of(file) {
            let label = format!("{}::{}", case.file.strip_prefix(conformance_dir()).unwrap().display(), case.entry);
            if !lowered.module.functions.contains_key(&case.entry) {
                continue;
            }
            let actual = match run(&lowered.module, &case.entry) {
                Err(why) if why.starts_with("unsupported") || why == "entry has parameters" => {
                    unsupported += 1;
                    *reasons.entry(why.chars().take(90).collect()).or_default() += 1;
                    continue;
                }
                Err(why) => {
                    failures.push(format!("{label}: {why}"));
                    continue;
                }
                Ok(r) => r,
            };
            let matched = match (&case.expect, &actual) {
                (Expect::Value(w), Ok(g)) => w == g,
                (Expect::Error(w), Err(g)) => g.contains(w.as_str()),
                _ => false,
            };
            if matched {
                ok += 1;
            } else {
                failures.push(format!("{label}: expected {:?}, got {actual:?}", case.expect));
            }
        }
    }
    eprintln!("wasm conformance: {ok} agree, {unsupported} not compiled to wasm, {} differ", failures.len());
    let mut by_count: Vec<_> = reasons.iter().collect();
    by_count.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in by_count.iter().take(25) {
        eprintln!("  {n:4} x {why}");
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
