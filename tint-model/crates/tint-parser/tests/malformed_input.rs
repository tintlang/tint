// Regression tests for a real bug: `Parser::parse_program`'s symbol
// pre-scan pass (see `parse_program`'s first block in parser.rs) walks
// the token stream with `skip_fn_decl`/`skip_struct_decl`/`skip_enum_decl`,
// which in turn use depth-counted brace/paren/bracket/angle "skip until
// matching close" loops (skip_parens.rs). Those loops used to have no
// EOF check at all: `TokenStream::next()` at end-of-input keeps
// returning the same Eof token forever without advancing its position
// (that's intentional elsewhere, so callers can peek past the end
// safely), so a depth counter that never reaches zero -- because the
// source is missing a closing delimiter -- spun forever. In practice
// this meant any source with an unbalanced brace (e.g. someone pasting
// a UI block twice without closing the outer one) hung the whole
// process/tab instead of producing a parse error. Fixed by making each
// of those loops bail with a proper `ParserError` the moment it sees
// `TokenKind::Eof`.
//
// Every case below must return an `Err` promptly. If any of these
// regress to the old behavior, the test process itself hangs (rather
// than failing cleanly), which is still a clear enough signal in CI.

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;

fn parse(src: &str) -> Result<(), String> {
    let tokens = collect_tokens(&mut Lexer::new(src));
    let mut parser = Parser::new(tokens);
    match parser.parse_program() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{:?}", e)),
    }
}

#[test]
fn unclosed_brace_in_ui_fn_body_errors_instead_of_hanging() {
    // The user's exact reproduction: a `Row { ... }` block pasted twice
    // without the outer `ui fn`'s own closing `}`.
    let src = r#"
        ui fn App() {
            Row {
                direction::row
                align::center
                gap::8
            Row {
                direction::row
                align::center
                gap::8
            }
        }
    "#;

    let err = parse(src).expect_err("unbalanced braces must be a parse error, not Ok");
    assert!(err.contains("unclosed"), "unexpected error: {err}");
}

#[test]
fn ui_theme_tokens_and_references_parse() {
    parse(
        r##"
        ui fn App() {
            theme::dark {
                tokens {
                    text-main::#ffffff
                    button-size::18
                }
            }
            Button {
                text::{16, @text-main}
                layout::{ padding.x::@button-size }
                "Open"
            }
        }
        "##,
    )
    .expect("theme tokens and @ references should parse");
}

#[test]
fn ui_style_declarations_and_use_parse() {
    parse(
        r##"
        ui fn App() {
            style ButtonBase {
                layout::{ padding.x::12 }
                motion::{ hover::{ scale::1.04 } }
            }
            Button { use::ButtonBase "Open" }
        }
        "##,
    )
    .expect("style declarations and use should parse");
}

#[test]
fn ui_components_and_default_and_named_slots_parse() {
    parse(
        r#"
        ui fn App() {
            component Card {
                layout::{ direction::column }
                variant::raised { paint::{ background::#ffffff } }
                slot::header
                slot::content
            }
            Card {
                slot header { Text { "Title" } }
                Text { "Body" }
            }
        }
        "#,
    )
    .expect("components and slots should parse");
}

#[test]
fn unclosed_paren_in_plain_fn_errors() {
    let err = parse("fn foo(\n").expect_err("must error");
    assert!(err.contains("unclosed `(`"), "unexpected error: {err}");
}

#[test]
fn unclosed_brace_in_struct_errors() {
    let err = parse("struct Foo {\n    x: i32\n").expect_err("must error");
    assert!(err.contains("unclosed `{`"), "unexpected error: {err}");
}

#[test]
fn unclosed_paren_in_enum_variant_errors() {
    let err = parse("enum Foo {\n    A,\n    B(\n").expect_err("must error");
    assert!(err.contains("unclosed"), "unexpected error: {err}");
}

#[test]
fn unclosed_angle_bracket_in_generics_errors() {
    let err = parse("fn foo<T {\n}\n").expect_err("must error");
    assert!(err.contains("unclosed `<`"), "unexpected error: {err}");
}
