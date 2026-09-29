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
fn numeric_casts_preserve_runtime_types() {
    let value = run(r#"
fn test() {
    let count: u8 = 42
    let ratio: f32 = count as f32
    ratio as f64
}

"#);

    assert!(matches!(value, Value::F64(value) if value == 42.0));
}

#[test]
fn all_integer_widths_have_distinct_runtime_values() {
    assert!(matches!(run("fn test() { 1 as i32 }"), Value::I32(1)));
    assert!(matches!(run("fn test() { 1 as i64 }"), Value::I64(1)));
    assert!(matches!(run("fn test() { 1 as u32 }"), Value::U32(1)));
    assert!(matches!(run("fn test() { 1 as u64 }"), Value::U64(1)));
    assert!(matches!(run("fn test() -> i32 { 1 }"), Value::I32(1)));
}

#[test]
fn typed_constants_and_parameters_keep_their_types() {
    assert!(matches!(
        run("const LIMIT: u8 = 255 fn test() -> u8 { LIMIT }"),
        Value::U8(255)
    ));
    assert!(matches!(
        run("fn take(value: i64) -> i64 { value } fn test() -> i64 { take(7) }"),
        Value::I64(7)
    ));
}

#[test]
fn typed_numeric_arithmetic_keeps_the_wider_runtime_type() {
    let value = run(r#"
fn test() {
    let count: u8 = 40
    let ratio: f32 = count as f32
    let sum: f32 = ratio + 2.0
    sum as f64
}
"#);

    assert!(matches!(value, Value::F64(value) if value == 42.0));
}

#[test]
fn u8_cast_rejects_fractional_and_out_of_range_values() {
    for source in [
        "fn test() { 1.5 as u8 }",
        "fn test() { 256 as u8 }",
        "fn test() { -1 as u32 }",
        "fn test() { 2147483648 as i32 }",
    ] {
        let mut vm = TintVM::new();
        let tokens = collect_tokens(&mut Lexer::new(source));
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().expect("source should parse");
        vm.run_program(&program);
        let error = vm
            .call_fn("test", &[], tint_ast::Span::dummy())
            .expect_err("invalid cast should fail");
        assert!(error.to_string().contains("numeric cast failed"));
    }
}

#[test]
fn typed_arithmetic_overflow_is_a_runtime_error() {
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new(
        "fn test() -> u8 { let value: u8 = 255 value + 1 }",
    ));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
    let error = vm
        .call_fn("test", &[], tint_ast::Span::dummy())
        .expect_err("overflow should fail");
    assert!(error.to_string().contains("numeric return type error"));
}
