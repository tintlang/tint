use tint_evaluator::value::Value;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn run(source: &str) -> Value {
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
    vm.call_fn("test", &[], tint_ast::Span::dummy())
        .expect("function should run")
}

fn number(value: Value) -> f64 {
    match value {
        Value::Number(n) => n,
        other => panic!("expected number, got {other:?}"),
    }
}

fn string(value: Value) -> String {
    match value {
        Value::String(s) => s,
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn list_len_push_pop_remove_write_back_to_the_variable() {
    let value = run(r#"
fn test() {
    let mut xs = [10, 20]
    xs.push(30)
    xs.push(40)
    xs.pop()
    xs.remove(0)
    xs.len()
}
"#);
    assert_eq!(number(value), 2.0);
}

#[test]
fn list_pop_and_remove_return_values() {
    let value = run(r#"
fn test() {
    let mut xs = [1, 2, 3]
    let last = xs.pop().unwrap()
    let first = xs.remove(0)
    last * 10 + first
}
"#);
    assert_eq!(number(value), 31.0);
}

#[test]
fn list_pop_on_empty_is_none() {
    let value = run(r#"
fn test() {
    let mut xs = [1]
    xs.pop()
    if xs.pop().is_none() && xs.is_empty() { 1 } else { 0 }
}
"#);
    assert_eq!(number(value), 1.0);
}

#[test]
fn list_map_filter_with_lambdas() {
    let value = run(r#"
fn test() {
    let xs = [1, 2, 3, 4, 5, 6]
    let evens = xs.filter(|x| x % 2 == 0)
    let doubled = evens.map(|x| x * 2)
    doubled.len() * 100 + doubled.join(",").len()
}
"#);
    // [4, 8, 12] -> len 3, "4,8,12" -> 6 chars
    assert_eq!(number(value), 306.0);
}

#[test]
fn list_join_and_contains() {
    let value = run(r#"
fn test() {
    let names = ["ann", "bob"]
    if names.contains("bob") && !names.contains("cy") {
        names.join(" & ")
    } else {
        "wrong"
    }
}
"#);
    assert_eq!(string(value), "ann & bob");
}

#[test]
fn string_methods() {
    let value = run(r#"
fn test() {
    let s = "  Hello, World  "
    let t = s.trim()
    let parts = t.to_lower().split(", ")
    let ok = t.contains("World") && t.starts_with("Hello") && t.ends_with("World")
    if ok && t.len() == 12 && parts.len() == 2 {
        parts.join("|") + "/" + t.to_upper() + "/" + t.replace("l", "L")
    } else {
        "wrong"
    }
}
"#);
    assert_eq!(string(value), "hello|world/HELLO, WORLD/HeLLo, WorLd");
}

#[test]
fn string_split_empty_separator_gives_characters() {
    let value = run(r#"
fn test() {
    "abc".split("").len()
}
"#);
    assert_eq!(number(value), 3.0);
}

#[test]
fn map_methods() {
    let value = run(r#"
fn test() {
    let mut m = map { b{2}, a{1} }
    m.set("c", 3)
    m.remove("b")
    let missing = m.get("zzz").is_none()
    let a = m.get("a").unwrap()
    if m.has("c") && !m.has("b") && missing {
        m.keys().join(",") + ":" + a + ":" + m.len()
    } else {
        "wrong"
    }
}
"#);
    assert_eq!(string(value), "a,c:1:2");
}

#[test]
fn unknown_method_is_an_error_not_a_silent_unit() {
    let mut vm = TintVM::new();
    let source = "fn test() { [1, 2].nope() }";
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().unwrap();
    vm.run_program(&program);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vm.call_fn("test", &[], tint_ast::Span::dummy())
    }));
    assert!(matches!(result, Err(_) | Ok(Err(_))));
}

#[test]
fn method_call_binds_tighter_than_prefix_operators() {
    let value = run(r#"
fn test() {
    let xs = [1, 2]
    let empty = []
    if !xs.is_empty() && empty.is_empty() { -xs.len() } else { 0 }
}
"#);
    assert_eq!(number(value), -2.0);
}

#[test]
fn cast_still_binds_looser_than_prefix_minus() {
    let value = run("fn test() { let x = 5; -x as i32 }");
    assert!(matches!(value, Value::I32(-5)));
}

#[test]
fn modulo_works_in_both_execution_paths() {
    assert_eq!(number(run("fn test() { 7 % 4 }")), 3.0);
    assert_eq!(number(run("fn test() { let f = |x| x % 4; f(7) }")), 3.0);
}

#[test]
fn string_concatenation_inside_a_lambda() {
    let value = run(r#"
fn test() {
    ["a", "b"].map(|s| "<" + s + ">").join("")
}
"#);
    assert_eq!(string(value), "<a><b>");
}

#[test]
fn interpolation_accepts_string_literals_and_braces_inside_them() {
    let value = run(r#"
fn test() {
    let xs = ["a", "b"]
    let s = "q"
    "{xs.join(" ")}|{s.replace("q", "}")}|é{xs.len()}é"
}
"#);
    assert_eq!(string(value), "a b|}|é2é");
}

#[test]
fn string_escapes() {
    let value = run(r#"fn test() { "a\nb\t\"c\"\\d" }"#);
    assert_eq!(string(value), "a\nb\t\"c\"\\d");
}

#[test]
fn list_sort_reverse_slice_find_return_new_lists() {
    let value = run(r#"
fn test() {
    let xs = [3, 1, 2]
    let sorted = xs.sort()
    let rev = sorted.reverse()
    let mid = rev.slice(1)
    let head = rev.slice(0, 2)
    let words = ["pear", "apple", "fig"].sort()
    let found = xs.find(|x| x > 1).unwrap()
    let none = xs.find(|x| x > 9).is_none()
    if none && xs.join(",") == "3,1,2" {
        sorted.join("") + "|" + rev.join("") + "|" + mid.join("") + "|" + head.join("") + "|" + words.join(" ") + "|" + found
    } else {
        "wrong"
    }
}
"#);
    assert_eq!(string(value), "123|321|21|32|apple fig pear|3");
}

#[test]
fn slice_is_clamped_and_sort_rejects_mixed_lists() {
    assert_eq!(
        number(run(
            "fn test() { [1, 2, 3].slice(5).len() + [1, 2, 3].slice(2, 1).len() }"
        )),
        0.0
    );
    assert_eq!(string(run("fn test() { \"héllo\".slice(1, 3) }")), "él");
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new("fn test() { [1, \"a\"].sort() }"));
    let program = Parser::new(tokens).parse_program().unwrap();
    vm.run_program(&program);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vm.call_fn("test", &[], tint_ast::Span::dummy())
    }));
    assert!(matches!(result, Err(_) | Ok(Err(_))));
}
