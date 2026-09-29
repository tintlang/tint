//! Engine-agnostic conformance suite.
//!
//! Every `conformance/*.tn` file holds any number of cases. A case is a
//! directive comment directly above a zero-argument `fn`:
//!
//! ```text
//! // expect: 42            result must render to exactly `42`
//! // expect-error: text    running must fail, message must contain `text`
//! // xfail: reason         known gap: must currently FAIL (flip to `expect`
//! //                       once fixed; a passing xfail fails the suite)
//! fn sum() { 40 + 2 }
//! ```
//!
//! `xfail` is placed on its own line between the `expect` line and the `fn`.
//! Each case runs on a fresh VM with the whole file loaded, so cases may
//! share helper functions declared in the same file.
//!
//! To test another engine, implement `Engine` and add it to `engines()`;
//! every engine must render every case identically.

use std::fs;
use std::path::{Path, PathBuf};

use tint_evaluator::value::Value;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

trait Engine {
    fn name(&self) -> &'static str;
    /// Run `entry` (zero args) from `source`; return the rendered result or an error message.
    fn run(&self, source: &str, entry: &str) -> Result<String, String>;
}

struct TreeWalker;

impl Engine for TreeWalker {
    fn name(&self) -> &'static str {
        "tree-walker"
    }

    fn run(&self, source: &str, entry: &str) -> Result<String, String> {
        let source = source.to_owned();
        let entry = entry.to_owned();
        let outcome = std::panic::catch_unwind(move || {
            let mut vm = TintVM::new();
            let tokens = collect_tokens(&mut Lexer::new(&source));
            let program = Parser::new(tokens)
                .parse_program()
                .map_err(|e| format!("parse error: {e:?}"))?;
            vm.run_program(&program);
            vm.call_fn(&entry, &[], tint_ast::Span::dummy())
                .map(|v| render(&v))
                .map_err(|e| format!("{e:?}"))
        });
        match outcome {
            Ok(result) => result,
            Err(payload) => Err(payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "panic".into())),
        }
    }
}

fn engines() -> Vec<Box<dyn Engine>> {
    vec![Box::new(TreeWalker)]
}

/// Canonical text form shared by all engines.
fn render(value: &Value) -> String {
    match value {
        Value::Number(n) | Value::F64(n) => format!("{n}"),
        Value::I32(n) => format!("{n}i32"),
        Value::I64(n) => format!("{n}i64"),
        Value::U32(n) => format!("{n}u32"),
        Value::U64(n) => format!("{n}u64"),
        Value::U8(n) => format!("{n}u8"),
        Value::F32(n) => format!("{n}f32"),
        Value::String(s) => format!("{s:?}"),
        Value::Bool(b) => b.to_string(),
        Value::Unit => "()".into(),
        Value::Propagate(inner) => render(inner),
        Value::Tuple(items) => format!("({})", join(items)),
        Value::List(items) => format!("[{}]", join(items)),
        Value::StructInstance { name, fields } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", render(v)))
                .collect();
            format!("{name} {{ {} }}", fields.join(", "))
        }
        Value::EnumInstance {
            enum_name,
            variant,
            args,
        } if args.is_empty() => format!("{enum_name}::{variant}"),
        Value::EnumInstance {
            enum_name,
            variant,
            args,
        } => format!("{enum_name}::{variant}({})", join(args)),
        Value::Map(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{k}: {}", render(&map[k])))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        _ => "<function>".into(),
    }
}

fn join(items: &[Value]) -> String {
    items.iter().map(render).collect::<Vec<_>>().join(", ")
}

#[derive(Debug)]
enum Expectation {
    Value(String),
    Error(String),
}

#[derive(Debug)]
struct Case {
    file: String,
    entry: String,
    expectation: Expectation,
    xfail: Option<String>,
}

fn parse_cases(path: &Path) -> (String, Vec<Case>) {
    let source = fs::read_to_string(path).unwrap();
    let file = path.file_name().unwrap().to_string_lossy().into_owned();
    let mut cases = Vec::new();
    let mut pending: Option<Expectation> = None;
    let mut xfail: Option<String> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// expect-error:") {
            pending = Some(Expectation::Error(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// expect:") {
            pending = Some(Expectation::Value(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// xfail:") {
            xfail = Some(rest.trim().to_owned());
        } else if let Some(rest) = trimmed.strip_prefix("fn ") {
            if let Some(expectation) = pending.take() {
                let entry: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                cases.push(Case {
                    file: file.clone(),
                    entry,
                    expectation,
                    xfail: xfail.take(),
                });
            }
            xfail = None;
        }
    }
    (source, cases)
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

#[test]
fn conformance_suite() {
    let mut files: Vec<PathBuf> = fs::read_dir(conformance_dir())
        .expect("conformance directory")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tn"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no conformance files found");

    let mut failures = Vec::new();
    let mut total = 0;
    let mut xfailed = 0;
    for engine in engines() {
        for path in &files {
            let (source, cases) = parse_cases(path);
            for case in cases {
                total += 1;
                let actual = engine.run(&source, &case.entry);
                let matched = match (&case.expectation, &actual) {
                    (Expectation::Value(want), Ok(got)) => want == got,
                    (Expectation::Error(want), Err(got)) => got.contains(want.as_str()),
                    _ => false,
                };
                let label = format!("[{}] {}::{}", engine.name(), case.file, case.entry);
                match (&case.xfail, matched) {
                    (None, true) => {}
                    (None, false) => failures.push(format!(
                        "{label}: expected {:?}, got {actual:?}",
                        case.expectation
                    )),
                    (Some(_), false) => {
                        xfailed += 1;
                        eprintln!("xfail {label}: got {actual:?}");
                    }
                    (Some(reason), true) => failures.push(format!(
                        "{label}: xfail ({reason}) now passes; change it to a plain expect"
                    )),
                }
            }
        }
    }
    eprintln!("conformance: {total} cases, {xfailed} known gaps (xfail)");
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
