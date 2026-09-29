use tint_evaluator::value::Value;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn run(source: &str, function: &str) -> Value {
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
    vm.call_fn(function, &[], tint_ast::Span::dummy())
        .expect("function should run")
}

#[test]
fn option_and_result_variants_are_real_runtime_values() {
    let value = run(
        r#"
fn test() {
    let option: Option<f32> = Option::Some { value: 41.0 }
    let result: Result<f32, string> = Result::Ok { value: 7.0 }
    option.expect("missing") + result.unwrap()
}
"#,
        "test",
    );

    assert!(matches!(value, Value::Number(value) if value == 48.0));
}

#[test]
fn question_mark_extracts_success_and_returns_failure() {
    let value = run(
        r#"
fn ok_value() -> Result<f32, string> {
    let value = Result::Ok { value: 41.0 }?
    Result::Ok { value: value + 1.0 }
}

fn failed_value() -> Result<f32, string> {
    let value = Result::Err { error: "bad input" }?
    Result::Ok { value: value }
}

fn test() {
    failed_value()?
    1.0
}
"#,
        "test",
    );

    assert!(
        matches!(value, Value::EnumInstance { enum_name, variant, .. } if enum_name == "Result" && variant == "Err")
    );
    let success = run(
        r#"fn test() -> Result<f32, string> { let value = ok_value()? Result::Ok { value: value } } fn ok_value() -> Result<f32, string> { let value = Result::Ok { value: 41.0 }? Result::Ok { value: value + 1.0 } }"#,
        "test",
    );
    assert!(
        matches!(success, Value::EnumInstance { enum_name, variant, args } if enum_name == "Result" && variant == "Ok" && matches!(args.first(), Some(Value::Number(value)) if *value == 42.0))
    );
}

#[test]
fn failed_expect_is_reported_as_runtime_error() {
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new(
        r#"fn test() { Option::None {}.expect("value is required") }"#,
    ));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);

    let error = vm
        .call_fn("test", &[], tint_ast::Span::dummy())
        .expect_err("expect failure should be a runtime error");
    assert!(error.to_string().contains("expect called"));
}
