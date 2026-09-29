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
fn unit_enum_variants_work_as_game_states() {
    let result = run(
        r#"
enum GameState {
    Menu,
    Playing,
    Paused,
}

fn label(current) {
    match current {
        Menu => "menu",
        Playing => "playing",
        Paused => "paused",
    }
}

fn test() {
    label(GameState::Paused)
}
"#,
        "test",
    );

    assert!(matches!(result, Value::String(value) if value == "paused"));
}

#[test]
fn tuple_enum_variants_work_as_game_events() {
    let result = run(
        r#"
enum GameEvent {
    Move(i32, i32),
    Quit,
}

fn distance(event) {
    match event {
        Move(x, y) => x * x + y * y,
        Quit => 0,
    }
}

fn test() {
    distance(GameEvent::Move(3, 4))
}
"#,
        "test",
    );

    assert!(matches!(result, Value::Number(value) if value == 25.0));
}
