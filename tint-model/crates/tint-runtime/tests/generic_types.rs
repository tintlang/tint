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
fn generic_struct_and_enum_values_run_with_concrete_payloads() {
    let value = run(r#"
struct Box<T> { value: T }
enum Response<T, E> { Ok { value: T }, Err { error: E } }

fn test() {
    let boxed: Box<i32> = Box { value: 42 }
    let response: Response<i32, string> = Response::Ok { value: boxed.value }
    match response {
        Ok { value } => value,
        Err { error } => 0,
    }
}
"#);

    assert!(matches!(value, Value::Number(value) if value == 42.0));
}
