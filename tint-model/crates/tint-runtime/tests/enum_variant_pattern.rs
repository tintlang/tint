// Proves the fix to a real gap: a struct-STYLE enum variant (named
// field, `enum E { A { x } }`, as opposed to `enum E { A(x) }`) could be
// constructed but never pattern-matched. `A { x } => ...` parses to
// `Pattern::Struct` (tint-parser's `parse_ident_based_pattern` can't tell
// a struct name from a variant name apart at a bare `Name {` pattern
// site, so it always produces this), and `Pattern::Struct` used to only
// ever match `Value::StructInstance` -- never `Value::EnumInstance` -- so
// this pattern silently fell through to `_`/wildcard every time,
// regardless of the scrutinee's actual variant.
//
// Fixed in `tint-ir/src/ir_vm.rs`: `Pattern::Struct` now accepts either
// value kind (a `StructInstance` matched by struct name, an
// `EnumInstance` matched by variant name), which needed
// `Value::EnumInstance` (tint-ir/src/ir.rs) to retain field NAMES instead
// of a positional-only `Vec<Value>` -- mirroring `StructInstance`'s
// `fields: Vec<(String, Value)>`. `Instr::VariantInit` already carried
// those names at compile time; only the runtime value was throwing them
// away. Scoped to this crate's own `Value` type -- `tint-evaluator` (the
// tree-walking path used for UI handlers) has its own separate `Value`
// and pattern-matching code, untouched by this change, with the
// equivalent restriction still open there.

use tint_evaluator::value::Value;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn parse_and_run(code: &str, vm: &mut TintVM) {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
}

fn expect_number(v: Value) -> f64 {
    match v {
        Value::Number(n) => n,
        other => panic!("expected a Number, got {:?}", other),
    }
}

#[test]
fn a_struct_style_variant_pattern_matches_by_variant_name_and_binds_its_field() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
enum Shape {
    Circle { r },
    Square { side },
}

fn area(shape) {
    match shape {
        Circle { r } => r * r * 3,
        Square { side } => side * side,
        _ => 0,
    }
}

fn go() {
    let c = Shape::Circle { r{2} };
    area(c)
}
"#,
        &mut vm,
    );

    let result = vm
        .call_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    // Before the fix: `Circle { r }` (a `Pattern::Struct`) could never
    // match `Shape::Circle { r{2} }` (a `Value::EnumInstance`), so this
    // always fell through to `_ => 0` no matter the shape.
    assert_eq!(expect_number(result), 12.0); // 2*2*3
}

#[test]
fn a_struct_style_variant_pattern_picks_the_matching_variant_not_just_any() {
    // Same shapes as above, but calling with the OTHER variant -- proves
    // this is picking the arm by variant name, not just happening to
    // return the first arm's body regardless of which variant it is.
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
enum Shape {
    Circle { r },
    Square { side },
}

fn area(shape) {
    match shape {
        Circle { r } => r * r * 3,
        Square { side } => side * side,
        _ => 0,
    }
}

fn go() {
    let s = Shape::Square { side{5} };
    area(s)
}
"#,
        &mut vm,
    );

    let result = vm
        .call_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 25.0); // 5*5
}

// NOTE: there's no positional-declaration counterpart test here
// (`enum Pair { Two(a, b) }` matched with `Two(x, y) => ..`) -- the
// semantic checker rejects tuple-style variant DECLARATIONS outright
// ("Tuple variants like Foo(T) are not allowed in Tint"), so every real
// enum variant in this language is the named-field, struct-style shape
// this file already covers. `Pattern::Variant` (the parenthesis-style
// positional pattern `A(x) => ..`, still handled in `pattern_matches`
// unchanged by this fix) has no way to be exercised against an
// actually-declarable variant, so it's effectively dead code today,
// left alone rather than removed since it isn't this fix's concern.
