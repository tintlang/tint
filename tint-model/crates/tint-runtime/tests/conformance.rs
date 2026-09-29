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
//! Directives sit on their own lines between the `expect` line and the `fn`.
//! Three more narrow them to one engine:
//!
//! ```text
//! // xfail-tree-walker: reason   known gap of the tree-walker only
//! // ir-expect: 42f32            what the typed IR renders instead (it is
//! //                             statically typed: `f32` stays `f32`)
//! // ir-expect-error: text       the typed IR rejects the program or fails
//! //                             with this message instead
//! ```
//!
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
    /// Statically typed engines may render differently where the tree-walker
    /// is loose about number types (see `ir-expect`).
    fn typed_ir(&self) -> bool {
        false
    }
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

/// Parse, check strictly, lower to the typed IR and run it on the reference
/// interpreter.
struct TypedIr;

impl Engine for TypedIr {
    fn name(&self) -> &'static str {
        "typed-ir"
    }

    fn typed_ir(&self) -> bool {
        true
    }

    fn run(&self, source: &str, entry: &str) -> Result<String, String> {
        let source = source.to_owned();
        let entry = entry.to_owned();
        // Deep recursion in the interpreter needs more than a test thread's stack.
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(move || run_typed_ir(&source, &entry))
            .expect("spawn");
        handle.join().unwrap_or_else(|_| Err("panic".into()))
    }
}

fn run_typed_ir(source: &str, entry: &str) -> Result<String, String> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("parse error: {e:?}"))?;
    let ctx = tint_semantics::prelude::CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = tint_semantics::SemanticChecker::new(ctx).check_with_model(&program);
    if !errors.is_empty() {
        return Err(format!("type error: {errors:?}"));
    }
    let lowered = tint_ir::typed::lower_program(&program, &model);
    if !lowered.errors.is_empty() {
        let messages: Vec<String> = lowered.errors.iter().map(|e| e.to_string()).collect();
        return Err(format!("lowering error: {}", messages.join("; ")));
    }
    let mut interp = tint_ir::typed::Interp::new(&lowered.module);
    interp.steps_left = Some(200_000_000);
    let (value, ty) = interp.run(entry, Vec::new()).map_err(|trap| trap.to_string())?;
    Ok(tint_ir::typed::interp::render(&lowered.module, ty, &value))
}

fn engines() -> Vec<Box<dyn Engine>> {
    vec![Box::new(TreeWalker), Box::new(TypedIr)]
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
    /// Overrides `expectation` for the statically typed IR.
    ir_expectation: Option<Expectation>,
    /// Known gap of every engine.
    xfail: Option<String>,
    /// Known gap of the tree-walker only.
    xfail_tree_walker: Option<String>,
}

fn parse_cases(path: &Path) -> (String, Vec<Case>) {
    let source = fs::read_to_string(path).unwrap();
    let file = path.strip_prefix(conformance_dir()).unwrap_or(path).to_string_lossy().into_owned();
    let mut cases = Vec::new();
    let mut pending: Option<Expectation> = None;
    let mut xfail: Option<String> = None;
    let mut xfail_tree_walker: Option<String> = None;
    let mut ir_expectation: Option<Expectation> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("// expect-error:") {
            pending = Some(Expectation::Error(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// expect:") {
            pending = Some(Expectation::Value(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// ir-expect-error:") {
            ir_expectation = Some(Expectation::Error(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// ir-expect:") {
            ir_expectation = Some(Expectation::Value(rest.trim().to_owned()));
        } else if let Some(rest) = trimmed.strip_prefix("// xfail-tree-walker:") {
            xfail_tree_walker = Some(rest.trim().to_owned());
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
                    ir_expectation: ir_expectation.take(),
                    xfail: xfail.take(),
                    xfail_tree_walker: xfail_tree_walker.take(),
                });
            }
            xfail = None;
            xfail_tree_walker = None;
            ir_expectation = None;
        }
    }
    (source, cases)
}

fn conformance_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

/// Every `.tn` file under `dir`, category folders included.
fn collect_cases(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("conformance directory") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_cases(&path, out);
        } else if path.extension().is_some_and(|e| e == "tn") {
            out.push(path);
        }
    }
}

#[test]
fn conformance_suite() {
    let mut files = Vec::new();
    collect_cases(&conformance_dir(), &mut files);
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
                let expectation = match (&case.ir_expectation, engine.typed_ir()) {
                    (Some(over), true) => over,
                    _ => &case.expectation,
                };
                let known_gap = if engine.typed_ir() {
                    case.xfail.as_ref()
                } else {
                    case.xfail.as_ref().or(case.xfail_tree_walker.as_ref())
                };
                let matched = match (expectation, &actual) {
                    (Expectation::Value(want), Ok(got)) => want == got,
                    (Expectation::Error(want), Err(got)) => got.contains(want.as_str()),
                    _ => false,
                };
                let label = format!("[{}] {}::{}", engine.name(), case.file, case.entry);
                match (known_gap, matched) {
                    (None, true) => {}
                    (None, false) => failures.push(format!(
                        "{label}: expected {expectation:?}, got {actual:?}"
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
