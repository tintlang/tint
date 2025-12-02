use rune_parser::Parser;
use rune_lexer::Lexer;
use rune_runtime::vm::RuneVM;

#[test]
fn test_logic_basic_arithmetic() {
    let code = r#"
        fn Calc() {
            let x = 5
            let y = 7
            return x * y + 3
        }
    "#;

    let mut lexer = Lexer::new(code);
    let tokens = rune_lexer::collect_tokens(&mut lexer);

    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parser failed");

    let mut vm = RuneVM::new();
    vm.run_program(&program);

    assert_eq!(vm.call_int("Calc"), 5 * 7 + 3);
}
