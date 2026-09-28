use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;

fn parse(source: &str) -> Result<tint_ast::Program, tint_parser::error::ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    Parser::new(tokens).parse_program()
}

#[test]
fn typed_parameter_defaults_use_equals() {
    parse(r#"fn greet(name: string, prefix: string = "Hello") { prefix }"#)
        .expect("typed defaults should parse");
}

#[test]
fn legacy_brace_defaults_still_parse() {
    parse(r#"fn greet(name, prefix {"Hello"}) { prefix }"#)
        .expect("legacy untyped brace defaults should parse");
    parse(r#"fn greet(name, prefix: string {"Hello"}) { prefix }"#)
        .expect("legacy typed brace defaults should parse");
}

#[test]
fn typed_and_tint_parameter_styles_cannot_be_mixed() {
    let err = parse(r#"fn greet(name: string, prefix {"Hello"}) { prefix }"#)
        .expect_err("mixed parameter styles should be rejected");
    assert!(
        format!("{err:?}").contains("Cannot mix"),
        "unexpected error: {err:?}"
    );
}
