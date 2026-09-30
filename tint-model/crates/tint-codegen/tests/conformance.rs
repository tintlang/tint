//! Differential test: every conformance case the native backend can compile
//! must give exactly what the typed-IR interpreter is expected to give.
//! Cases it cannot compile yet are counted, not failed.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Same directive syntax as `tint-runtime/tests/conformance.rs`; the typed
/// IR's expectation (`ir-expect`) wins, and known gaps (`xfail`) are skipped.
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
fn native_backend_agrees_with_the_interpreter() {
    let mut files = Vec::new();
    collect(&conformance_dir(), &mut files);
    files.sort();
    let (mut ok, mut unsupported, mut failures) = (0, 0, Vec::new());
    let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
    for file in &files {
        for case in cases_of(file) {
            let out = Command::new(env!("CARGO_BIN_EXE_tint-native-case"))
                .arg(&case.file)
                .arg(&case.entry)
                .output()
                .expect("run");
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let label = format!("{}::{}", case.file.strip_prefix(conformance_dir()).unwrap().display(), case.entry);
            let actual: Result<String, String> = if let Some(w) = stdout.lines().find_map(|l| l.strip_prefix("UNSUPPORTED ")) {
                unsupported += 1;
                let key: String = w.chars().take(90).collect();
                *reasons.entry(key).or_default() += 1;
                continue;
            } else if let Some(v) = stdout.lines().find_map(|l| l.strip_prefix("OK ")) {
                Ok(v.to_string())
            } else if let Some(e) = stdout.lines().find_map(|l| l.strip_prefix("ERR ")) {
                Err(e.to_string())
            } else if let Some(t) = stderr.lines().find_map(|l| l.strip_prefix("trap: ")) {
                Err(t.to_string())
            } else {
                failures.push(format!("{label}: crashed ({:?}) {stderr}", out.status));
                continue;
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
    eprintln!("native conformance: {ok} agree, {unsupported} not compiled natively, {} differ", failures.len());
    let mut by_count: Vec<_> = reasons.iter().collect();
    by_count.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (why, n) in by_count.iter().take(40) {
        eprintln!("  {n:4} x {why}");
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
