use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn run(source: &str) -> TintVM {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens)
        .parse_program()
        .expect("source should parse");
    let mut vm = TintVM::new();
    vm.run_program(&program);
    vm
}

#[test]
fn named_functions_can_be_passed_and_if_is_an_expression() {
    let mut vm = run(r#"
        fn is_positive(value: i32) -> bool { value > 0 }
        fn apply(predicate: fn(i32) -> bool, value: i32) -> bool {
            predicate(value)
        }
        fn choose(flag: bool) -> i32 {
            if flag { 1 } else { 2 }
        }
        fn test() { apply(is_positive, choose(true)) }
    "#);

    assert!(matches!(
        vm.call_fn("test", &[], tint_ast::Span::dummy()).unwrap(),
        Value::Bool(true)
    ));
}
