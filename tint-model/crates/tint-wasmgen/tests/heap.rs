//! Heap programs run on the wasm backend (with the runtime module) and on the
//! reference interpreter; results and traps must agree and nothing may leak.

mod common;
use common::run;
use tint_ir::typed::interp::render;
use tint_ir::typed::{lower_program, Interp, Module};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn lower(source: &str) -> Module {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    assert!(errors.is_empty(), "{errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
    lowered.module
}

fn reference(m: &Module, entry: &str) -> Result<String, String> {
    let mut interp = Interp::new(m);
    interp.init().map_err(|t| t.msg)?;
    match interp.run(entry, vec![]) {
        Ok((v, ty)) => Ok(render(m, ty, &v)),
        Err(t) => Err(t.msg),
    }
}

fn same(a: &Result<String, String>, b: &Result<String, String>) -> bool {
    match (a, b) {
        (Ok(x), Ok(y)) => x == y,
        (Err(x), Err(y)) => x.split(':').next() == y.split(':').next(),
        _ => false,
    }
}

fn check(source: &str) {
    let m = lower(source);
    let mut entries: Vec<&String> = m.functions.keys().filter(|n| n.starts_with("t_")).collect();
    entries.sort();
    assert!(!entries.is_empty());
    let mut bad = Vec::new();
    for e in entries {
        if std::env::var("TRACE").is_ok() { eprintln!("entry {e}"); }
        let want = reference(&m, e);
        match run(&m, e) {
            Err(why) => bad.push(format!("{e}: {why}")),
            Ok(got) => {
                if !same(&want, &got) {
                    bad.push(format!("{e}: interpreter {want:?}, wasm {got:?}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn strings() {
    check(
        r#"
fn t_concat() { let a = "foo"; let b = "bar"; "{a}-{b}-{a}" }
fn t_len() { "héllo".len() }
fn t_cmp() { let a = "abc"; let b = "abd"; if a < b && a != b && !(a == b) { 1 } else { 0 } }
fn t_loop() { let mut s = ""; for i in 0..50 { s = "{s}{i},"; }; s }
fn t_upper() { "hello".to_upper() + "WORLD".to_lower() }
fn t_split() { "a,b,c".split(",").join("+") }
fn t_slice() { "abcdef".slice(1, 4) }
fn t_replace() { "a-b-c".replace("-", "+") }
fn t_trim() { "  x  ".trim() }
fn t_contains() { "hello".contains("ell") && "hello".starts_with("he") && "hello".ends_with("lo") }
fn t_tostr() { let n = 3.5; let b = true; "{n} {b}" }
fn t_many() { let mut out = []; for i in 0..20 { out.push("item{i}") }; out.join("|") }
"#,
    );
}

#[test]
fn lists() {
    check(
        r#"
fn t_push() { let mut xs = []; for i in 0..100 { xs.push(i * 2) }; xs.len() }
fn t_index() { let xs = [10, 20, 30]; xs[1] + xs[2] }
fn t_set() { let mut xs = [1, 2, 3]; xs[1] = 99; xs }
fn t_cow() { let a = [1, 2, 3]; let mut b = a; b.push(4); b[0] = 7; "{a} {b}" }
fn t_cow2() { let a = [1, 2, 3]; let mut b = a; b[0] = 7; let c = a; "{a} {b} {c}" }
fn t_oob() { let xs = [1, 2, 3]; xs[5] }
fn t_oob_set() { let mut xs = [1, 2, 3]; xs[3] = 1; xs }
fn t_pop() { let mut xs = [1, 2, 3]; let a = xs.pop(); let b = xs.pop(); "{a} {b} {xs}" }
fn t_pop_empty() { let mut xs: Vec<number> = []; xs.pop().is_none() }
fn t_remove() { let mut xs = [1, 2, 3, 4]; xs.remove(1); xs }
fn t_reverse_sort() { let mut xs = [3, 1, 2]; xs.sort(); xs.reverse(); xs }
fn t_slice() { [1, 2, 3, 4, 5].slice(1, 3) }
fn t_contains() { [1, 2, 3].contains(2) }
fn t_join() { [1, 2, 3].join("-") }
fn t_for_in() { let mut s = 0; for x in [1, 2, 3, 4] { s = s + x }; s }
fn t_nested() { let mut g = [[1, 2], [3, 4]]; g[1][0] = 9; g }
fn t_i32_list() -> i32 { let xs: Vec<i32> = [1, 2, 3]; let mut s: i32 = 0; for x in xs { s = s + x }; s }
fn t_u8_list() { let xs: Vec<u8> = [250, 5, 255]; xs }
fn t_bool_list() { let mut xs = [true, false]; xs.push(true); xs }
fn t_str_list() { let mut xs = ["a", "b"]; xs.push("c"); xs[0] = "z"; xs }
fn t_sum() { let mut xs = []; for i in 0..1000 { xs.push(i) }; let mut s = 0; for x in xs { s = s + x }; s }
"#,
    );
}

#[test]
fn structs_enums_tuples() {
    check(
        r#"
struct P { x: number, y: number }
struct Line { a: P, b: P, name: string }
enum Shape { Circle { r: number }, Rect { w: number, h: number }, Empty {} }
fn area(s: Shape) {
    match s {
        Circle { r } => r * r * 3,
        Rect { w, h } => w * h,
        Empty {} => 0,
    }
}
fn t_struct() { let p = P { x: 1.0, y: 2.0 }; p.x + p.y }
fn t_struct_set() { let mut p = P { x: 1.0, y: 2.0 }; p.x = 10.0; p.y = p.y + p.x; p }
fn t_struct_cow() { let a = P { x: 1.0, y: 2.0 }; let mut b = a; b.x = 5.0; "{a.x} {b.x}" }
fn t_nested() { let mut l = Line { a: P { x: 1.0, y: 1.0 }, b: P { x: 2.0, y: 2.0 }, name: "n" }; l.a.x = 9.0; l.name = "m"; "{l.a.x} {l.b.x} {l.name}" }
fn t_nested_cow() { let l = Line { a: P { x: 1.0, y: 1.0 }, b: P { x: 2.0, y: 2.0 }, name: "n" }; let mut k = l; k.a.x = 9.0; "{l.a.x} {k.a.x}" }
fn t_enum() { area(Shape::Circle { r: 2.0 }) + area(Shape::Rect { w: 2.0, h: 3.0 }) + area(Shape::Empty {}) }
fn t_tuple() { let t = (1, "a", true); let (a, b, c) = t; "{a} {b} {c}" }
fn t_list_of_structs() {
    let mut ps = []
    for i in 0..10 { ps.push(P { x: i, y: i * 2 }) }
    ps[3].x = 100.0
    ps[4].y = ps[4].y + 1.0
    let mut s = 0
    for p in ps { s = s + p.x + p.y }
    s
}
fn t_list_of_structs_cow() {
    let a = [P { x: 1.0, y: 2.0 }, P { x: 3.0, y: 4.0 }]
    let mut b = a
    b[0].x = 50.0
    "{a[0].x} {b[0].x}"
}
fn t_option() {
    let m = map { a{1}, b{2} }
    let x = m.get("a")
    let y = m.get("z")
    match x { Some { value } => value, None {} => 0 } + match y { Some { value } => value, None {} => 100 }
}
"#,
    );
}

#[test]
fn maps_and_closures() {
    check(
        r#"
fn t_map() { let mut m = map { b{2}, a{1} }; m.set("c", 3); m.remove("b"); m.keys().join(",") }
fn t_map_get() { let m = map { a{1}, b{2} }; m.get("a").unwrap() + m["b"] }
fn t_map_missing() { let m = map { a{1} }; m["zzz"] }
fn t_map_cow() { let a = map { x{1} }; let mut b = a; b.set("x", 2); b.set("y", 3); "{a.len()} {b.len()} {a["x"]} {b["x"]}" }
fn t_map_str() { let mut m = map { a{"x"} }; m.set("b", "y"); m.values().join("") }
fn t_map_set_index() { let mut m = map { a{1} }; m["a"] = 5; m["b"] = 6; m.values() }
fn make_adder(n) { |x| x + n }
fn t_closure() { let f = make_adder(10); f(5) + f(6) }
fn t_closure_str() { let p = "hi "; let f = |x| "{p}{x}"; f("a") + f("b") }
fn apply(f, x) { f(f(x)) }
fn t_apply() { apply(|x| x * 2, 5) }
fn t_closure_list() { let xs = [1, 2, 3]; let f = || xs.len(); f() }
fn t_map_fn() { [1, 2, 3].map(|x| x * x) }
fn t_filter_fn() { [1, 2, 3, 4].filter(|x| x % 2 == 0) }
fn t_fold() { let mut total = 0; for x in [1, 2, 3].map(|x| x + 1) { total = total + x }; total }
fn counter() { let mut c = 0; let f = || c + 1; f() }
fn t_counter() { counter() }
"#,
    );
}

#[test]
fn globals_and_recursion() {
    check(
        r#"
const NAMES: Vec<string> = ["a", "b", "c"]
let greeting = "hello"
fn t_const_list() { NAMES.join("") }
fn t_global_str() { "{greeting} world" }
fn build(n) { if n == 0 { [] } else { let mut r = build(n - 1); r.push(n); r } }
fn t_rec_list() { build(30) }
fn rev(s, n) { if n == 0 { s } else { rev("{s}{n}", n - 1) } }
fn t_rec_str() { rev("", 20) }
fn t_early_return() { for x in ["a", "b", "c"] { if x == "b" { return x } }; "none" }
fn t_loop_alloc() { let mut n = 0; for i in 0..2000 { let xs = [i, i + 1, i + 2]; let s = "{xs}"; n = n + s.len() }; n }
"#,
    );
}

#[test]
fn text_helpers() {
    check(
        r#"
fn t_lines() { line_count("a\nbb\nccc") }
fn t_longest() { max_line_len("a\nbb\nccc") }
fn t_numbers() { line_numbers("a\nbb\nccc") }
fn t_highlight() { tint_highlight("fn f(x) = x + 1 // c") }
fn t_highlight_len() { tint_highlight("let a = 1 // c").len() }
fn t_first_class() { tint_highlight("fn f()")[0][1] }
"#,
    );
}
