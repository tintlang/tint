use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
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
        .expect("test function should run")
}

fn run_treewalk(source: &str) -> Value {
    let mut vm = TintVM::new();
    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
    vm.call_user_fn("test", &[], tint_ast::Span::dummy())
        .expect("test function should run")
}

#[test]
fn numeric_helpers_work_inside_an_ir_function() {
    let result = run(r#"
fn test() {
    clamp(-3, 0, 10) +
    clamp(20, 0, 10) +
    min(4, 9) +
    max(4, 9) +
    abs(-7) +
    sign(-12) +
    sign(0) +
    sign(12)
}
"#);

    assert!(matches!(result, Value::Number(value) if value == 30.0));
}

#[test]
fn vec2_supports_fields_and_methods_inside_an_ir_function() {
    let result = run(r#"
fn test() {
    let start = vec2(3, 4)
    let offset = vec2(1, 2)
    let moved = start.add(offset).scale(2)
    moved.x + moved.y + moved.length()
}
"#);

    // moved = (8, 12), then 8 + 12 + sqrt(208).
    assert!(
        matches!(result, Value::Number(value) if (value - (20.0 + 208.0_f64.sqrt())).abs() < 1e-9)
    );
}

#[test]
fn vec2_normalized_handles_zero_without_nan() {
    let result = run(r#"
fn test() {
    let value = vec2(0, 0).normalized()
    value.x + value.y
}
"#);

    assert!(matches!(result, Value::Number(value) if value == 0.0));
}

#[test]
fn vec2_operators_work_inside_an_ir_function() {
    let result = run(r#"
fn test() {
    let a = vec2(1, 2)
    let b = vec2(3, 4)
    let c = (a + b) * 2
    c.x + c.y
}
"#);

    assert!(matches!(result, Value::Number(value) if value == 20.0));
}

#[test]
fn vec2_operators_work_inside_a_treewalk_function() {
    let result = run_treewalk(
        r#"
fn test() {
    let a = vec2(8, 6)
    let b = vec2(3, 2)
    let c = (a - b) * 2
    c.x + c.y
}
"#,
    );

    assert!(matches!(result, Value::Number(value) if value == 18.0));
}
