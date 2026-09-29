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

#[test]
fn option_methods_cover_presence_fallback_and_mapping() {
    let value = run(r#"
fn test() {
    let some = Option::Some { value: 20.0 }
    let none = Option::None {}
    if some.is_some() && none.is_none() {
        some.map(|value| value + 22.0).unwrap_or(0.0)
    } else {
        0.0
    }
}
"#);

    assert!(matches!(value, Value::Number(value) if value == 42.0));
}

#[test]
fn result_methods_cover_status_fallback_and_and_then() {
    let value = run(r#"
fn test() {
    let ok = Result::Ok { value: 20.0 }
    let err = Result::Err { error: "bad" }
    if ok.is_ok() && err.is_err() {
        ok.and_then(|value| Result::Ok { value: value + 22.0 }).unwrap_or(0.0)
    } else {
        0.0
    }
}
"#);

    assert!(matches!(value, Value::Number(value) if value == 42.0));
}
