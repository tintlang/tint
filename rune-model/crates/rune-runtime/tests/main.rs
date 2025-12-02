use rune_parser::Parser;
use rune_lexer::Lexer;
use rune_runtime::vm::RuneVM;
use rune_runtime::state::store::StateStore;

#[test]
fn test_rune_runtime_e2e() {
    let code = r#"
        fn AddTest() {
            let a = 10
            let b = 20
            let c = a + b
        }

        ui fn Main() {
            <Text value="Hello Rune!" />
        }
    "#;

    println!("=== RUNE SOURCE CODE ===\n{}\n=========================", code);

    // LEXER
    let mut lexer = Lexer::new(code);
    let tokens = rune_lexer::collect_tokens(&mut lexer);

    // PARSER
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("Parser failed");

    // VM
    let mut vm = RuneVM::new();
    vm.run_program(&program);

    // Debug
    let c = vm.scopes.lookup("c");
    println!("Runtime: c = {:?}", c);

    assert!(vm.ui.root.is_some());
    println!("UI root node = {:?}", vm.ui.root);

    println!("E2E Runtime test finished.");
}
