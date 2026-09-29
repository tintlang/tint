// This crate had zero automated tests before this pass -- coverage of the
// checker's plain-`fn` existence-checking was previously exercised only ad
// hoc, via `tint check` runs against `examples/*.tn` from `tint-cli` (see
// that crate's own commit history). These tests specifically target the
// two areas widened in this pass: `ui fn` bodies and `impl` method bodies.

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::errors::SemanticErrorKind;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn errors_for(source: &str) -> Vec<SemanticErrorKind> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens)
        .parse_program()
        .unwrap_or_else(|e| panic!("source failed to parse: {:?}\n---\n{}", e, source));
    SemanticChecker::new(CheckerContext::default())
        .check(&program)
        .into_iter()
        .map(|e| e.kind)
        .collect()
}

#[test]
fn checker_exposes_inferred_expression_types_for_analyzer_clients() {
    let source = "fn test(value: i32) -> i32 { value + 1 }";
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().unwrap();
    let (errors, model) =
        tint_semantics::SemanticChecker::new(CheckerContext::default()).check_with_model(&program);

    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    assert!(model
        .expressions
        .iter()
        .any(|typed| matches!(typed.ty, tint_semantics::Type::Number)));
}

#[test]
fn user_generic_structs_and_enums_are_checked_strictly() {
    assert_clean(
        r#"
struct Box<T> { value: T }
enum Response<T, E> { Ok { value: T }, Err { error: E } }

fn test() {
    let boxed: Box<i32> = Box { value: 42 }
    let response: Response<i32, string> = Response::Ok { value: 42 }
}
"#,
    );
}

#[test]
fn user_generic_types_reject_wrong_arguments_and_arity() {
    let errors = errors_for(
        r#"
struct Box<T> { value: T }
enum Response<T, E> { Ok { value: T }, Err { error: E } }

fn test() {
    let wrong: Box<i32> = Box { value: "not an integer" }
    let bad_arity: Box<i32, string> = Box { value: 1 }
    let bad_enum: Response<i32, string> = Response::Ok { value: "wrong" }
}
"#,
    );

    assert!(errors
        .iter()
        .any(|error| matches!(error, SemanticErrorKind::TypeMismatch { .. })));
}

fn assert_clean(source: &str) {
    let errors = errors_for(source);
    assert!(
        errors.is_empty(),
        "expected no semantic errors, got {:?}\n---\n{}",
        errors,
        source
    );
}

fn assert_unknown_ident(source: &str, name: &str) {
    let errors = errors_for(source);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticErrorKind::UnknownIdent(n) if n == name)),
        "expected an UnknownIdent(\"{}\") among {:?}\n---\n{}",
        name,
        errors,
        source
    );
}

#[test]
fn ui_rejects_unknown_theme_token() {
    let errors = errors_for(
        r##"
        ui fn App() {
            theme::dark { tokens { text-main::white } }
            Text { text::{18, @missing-color} "Hello" }
        }
        "##,
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        SemanticErrorKind::UnknownUiToken(name) if name == "missing-color"
    )));
}

#[test]
fn ui_rejects_unknown_style() {
    let errors = errors_for(
        r#"
        ui fn App() {
            Button { use::MissingStyle "Open" }
        }
        "#,
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        SemanticErrorKind::UnknownUiStyle(name) if name == "MissingStyle"
    )));
}

#[test]
fn ui_rejects_unknown_variant() {
    let errors = errors_for(
        r#"
        ui fn App() {
            component Button { variant::solid { } slot::content }
            Button { variant::ghost "Open" }
        }
        "#,
    );
    assert!(errors.iter().any(|error| matches!(
        error,
        SemanticErrorKind::UnknownUiVariant(name) if name == "ghost"
    )));
}

fn assert_assign_to_immutable(source: &str, name: &str) {
    let errors = errors_for(source);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticErrorKind::AssignToImmutable(n) if n == name)),
        "expected an AssignToImmutable(\"{}\") among {:?}\n---\n{}",
        name,
        errors,
        source
    );
}

fn assert_type_mismatch(source: &str) {
    let errors = errors_for(source);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, SemanticErrorKind::TypeMismatch { .. })),
        "expected a TypeMismatch among {:?}\n---\n{}",
        errors,
        source
    );
}

fn assert_error(source: &str, predicate: impl Fn(&SemanticErrorKind) -> bool) {
    let errors = errors_for(source);
    assert!(
        errors.iter().any(predicate),
        "expected matching semantic error among {:?}\n---\n{}",
        errors,
        source
    );
}

#[test]
fn immutable_local_cannot_be_reassigned() {
    assert_assign_to_immutable(
        r#"
        fn update() {
            let value = 1
            value = 2
        }
        "#,
        "value",
    );
}

#[test]
fn mutable_local_can_be_reassigned() {
    assert_clean(
        r#"
        fn update() {
            let mut value = 1
            value = 2
        }
        "#,
    );
}

#[test]
fn type_checker_rejects_incompatible_assignment() {
    assert_type_mismatch(
        r#"
        fn update() {
            let mut value: i32 = 1
            value = "wrong"
        }
        "#,
    );
}

#[test]
fn consts_and_distinct_numeric_annotations_are_checked() {
    assert_clean(
        r#"
        const OFFSET: f32 = 32.0
        fn convert(value: f32) -> f32 { value + OFFSET }
        fn test() {
            let choice: u8 = 1
            convert(1)
        }
        "#,
    );
    assert_type_mismatch(
        r#"
        const OFFSET: f32 = 32.0
        fn test(value: u8) { let other: f32 = value }
        "#,
    );
}

#[test]
fn builtin_option_and_result_variants_are_typed() {
    assert_clean(
        r#"
        fn some(value: f32) -> Option<f32> {
            Option::Some { value: value }
        }
        fn none() -> Option<f32> {
            Option::None {}
        }
        fn ok(value: f32) -> Result<f32, string> {
            Result::Ok { value: value }
        }
        fn err(message: string) -> Result<f32, string> {
            Result::Err { error: message }
        }
        "#,
    );
    assert_type_mismatch(
        r#"
        fn wrong() -> Option<f32> {
            Option::Some { value: "not a number" }
        }
        "#,
    );
    assert_clean(
        r#"
        fn parsed() -> f32 {
            parse_number("42").expect("invalid number")
        }
        "#,
    );
}

#[test]
fn type_checker_rejects_wrong_function_argument() {
    assert_type_mismatch(
        r#"
        fn takes_number(value: i32) {}
        fn test() { takes_number("wrong") }
        "#,
    );
}

#[test]
fn type_checker_rejects_non_boolean_condition() {
    assert_type_mismatch(
        r#"
        fn test() { if 1 { "wrong" } }
        "#,
    );
}

#[test]
fn function_types_and_if_expressions_are_type_checked() {
    assert_clean(
        r#"
        fn is_positive(value: i32) -> bool { value > 0 }
        fn apply(predicate: fn(i32) -> bool, value: i32) -> bool {
            predicate(value)
        }
        fn choose(flag: bool) -> i32 {
            if flag { 1 } else { 2 }
        }
        fn test() { apply(is_positive, choose(true)) }
        "#,
    );
}

#[test]
fn type_checker_checks_explicit_return_type() {
    assert_type_mismatch(
        r#"
        fn wrong() -> string { return 1 }
        "#,
    );
}

#[test]
fn type_checker_checks_implicit_block_return_type() {
    assert_type_mismatch(
        r#"
        fn wrong() -> string { 1 }
        "#,
    );
}

#[test]
fn type_checker_checks_method_arguments_from_receiver_type() {
    assert_type_mismatch(
        r#"
        struct Vec2 { x: f32, y: f32 }
        impl Vec2 {
            fn scale(self, factor: f32) -> Vec2 { self }
        }
        fn test() {
            let value = Vec2 { x: 1, y: 2 }
            value.scale("wrong")
        }
        "#,
    );
}

#[test]
fn type_checker_checks_enum_constructor_arguments() {
    assert_type_mismatch(
        r#"
        enum Event { Tick(f32) }
        fn test() { Event::Tick("wrong") }
        "#,
    );
}

#[test]
fn enum_fields_accept_enum_constructors() {
    assert_clean(
        r#"
        enum GameState { Menu, Playing }
        struct Game { mode: GameState }
        fn test() { Game { mode: GameState::Menu } }
        "#,
    );
}

#[test]
fn type_checker_reports_non_exhaustive_enum_match() {
    assert_error(
        r#"
        enum GameState { Playing, Paused }
        fn label() { let mode = GameState::Playing; match mode { Playing => "playing" } }
        "#,
        |error| matches!(error, SemanticErrorKind::MissingMatchArms),
    );
}

#[test]
fn type_checker_rejects_a_pattern_with_the_wrong_type() {
    assert_type_mismatch(
        r#"
        fn label(value: i32) { match value { "wrong" => "bad", _ => "ok" } }
        "#,
    );
}

#[test]
fn type_checker_rejects_unknown_declaration_types() {
    assert_error(
        r#"
        struct User { profile: MissingProfile }
        "#,
        |error| matches!(error, SemanticErrorKind::UnknownIdent(name) if name == "MissingProfile"),
    );
}

#[test]
fn type_checker_infers_lambda_return_types() {
    assert_type_mismatch(
        r#"
        fn test() {
            let add_one = |value| value + 1
            let result: string = add_one(2)
        }
        "#,
    );
}

#[test]
fn ui_state_is_mutable_from_a_handler() {
    assert_clean(
        r#"
        fn increment() { count = count + 1 }
        ui fn App() {
            state count = 0
            Button { click||increment }
        }
        "#,
    );
}

#[test]
fn injected_viewport_width_is_visible_to_ui_expressions() {
    assert_clean(
        r#"
        ui fn App() {
            Text { if{viewport_width >= 640} "wide" }
        }
        "#,
    );
}

#[test]
fn injected_theme_is_visible_to_imported_handlers() {
    assert_clean(
        r#"
        fn set_dark() { theme = "dark" }
        ui fn App() {
            Text { "app" }
        }
        "#,
    );
}

// -- ui fn bodies --------------------------------------------------------

// A bare identifier as a plain modifier's VALUE (`padding::something`,
// `background::white`) parses to `UiModifierValue::Ident`, a symbolic
// token like a CSS keyword or color name -- NOT a variable read (there is
// no such thing as `let something = ...; padding::something` in this
// language). `UiModifierValue::Expr` only shows up for real control-flow
// (`if{}`/`for{}`/`match{}`) and `state` inits, which the tests below
// cover; there is deliberately no "undefined variable in an ordinary
// modifier" test, because that shape of bug does not exist in this
// grammar.

#[test]
fn ui_fn_flags_an_undefined_variable_in_a_text_interpolation() {
    assert_unknown_ident(
        r#"
        ui fn App() {
            Text { "{nope}" }
        }
        "#,
        "nope",
    );
}

#[test]
fn ui_fn_for_loop_variable_is_in_scope_for_its_children_only() {
    // The loop variable must be visible inside the `for{}`'s own children
    // (a real bug this pass would otherwise introduce: reusing the whole
    // generic modifier walk for `for{}`'s iterable without ever binding
    // `tx` would make every `for{tx in ...} Text { "{tx}" }` in the
    // codebase fail to check, including the real sandbox demo).
    assert_clean(
        r#"
        ui fn App() {
            Block { direction::column for{tx in [1, 2, 3]} Text { "{tx}" } }
        }
        "#,
    );
}

#[test]
fn ui_fn_for_loop_variable_does_not_leak_outside_its_own_node() {
    assert_unknown_ident(
        r#"
        ui fn App() {
            Block {
                Block { for{tx in [1, 2, 3]} Text { "{tx}" } }
                Text { "{tx}" }
            }
        }
        "#,
        "tx",
    );
}

#[test]
fn ui_fn_state_init_expression_is_checked() {
    assert_unknown_ident(
        r#"
        ui fn App() {
            state count = starting_value
            Text { "{count}" }
        }
        "#,
        "starting_value",
    );
}

#[test]
fn ui_fn_state_name_is_visible_to_a_plain_fn_used_as_a_click_handler() {
    // Mirrors the exact scenario `collect_top_level_names`'s own doc
    // comment calls out (confirmed there against `examples/ui_app.tn`):
    // `menu_open` is declared as UI state, then read/written by an
    // ordinary top-level `fn` that only ever gets called as a `click||`
    // handler at runtime -- never through the `ui fn` body itself.
    assert_clean(
        r#"
        fn toggle_menu() { menu_open = !menu_open }
        ui fn App() {
            state menu_open = false
            Button { click||toggle_menu }
        }
        "#,
    );
}

#[test]
fn ui_fn_if_condition_is_checked_and_branch_still_renders_regardless() {
    assert_unknown_ident(
        r#"
        ui fn App() {
            Block { if{missing_flag} Text { "shown" } }
        }
        "#,
        "missing_flag",
    );
}

#[test]
fn ui_fn_rejects_non_boolean_if_modifier() {
    assert_type_mismatch(
        r#"
        ui fn App() { Block { if{1} Text { "wrong" } } }
        "#,
    );
}

#[test]
fn ui_fn_rejects_an_unknown_event_handler() {
    assert_unknown_ident(
        r#"
        ui fn App() { Button { click||missing_handler "bad" } }
        "#,
        "missing_handler",
    );
}

// -- impl method bodies ---------------------------------------------------

#[test]
fn impl_method_can_reference_self_without_a_false_positive() {
    assert_clean(
        r#"
        struct Vec2 { x: f32, y: f32 }
        impl Vec2 {
            fn length_sq(self) { self.x * self.x + self.y * self.y }
        }
        "#,
    );
}

#[test]
fn impl_method_body_still_catches_a_genuinely_undefined_variable() {
    assert_unknown_ident(
        r#"
        struct Vec2 { x: f32, y: f32 }
        impl Vec2 {
            fn broken(self) { self.x + totally_undefined }
        }
        "#,
        "totally_undefined",
    );
}

#[test]
fn impl_method_params_besides_self_are_bound_normally() {
    assert_clean(
        r#"
        struct Vec2 { x: f32, y: f32 }
        impl Vec2 {
            fn scaled(self, factor) { self.x * factor }
        }
        "#,
    );
}

#[test]
fn list_string_and_map_methods_are_typed() {
    assert_clean(
        r#"
fn test() {
    let mut xs = [1, 2, 3]
    xs.push(4)
    let n: i32 = xs.len()
    let last = xs.pop().unwrap_or(0)
    let names = xs.map(|x| "n{x}")
    let joined: string = names.join(",")
    let parts = joined.split(",")
    let evens = xs.filter(|x| x % 2 == 0)
    let m = map { a{1} }
    let keys = m.keys().join("|")
    let v = m.get("a").unwrap_or(0)
    let ok = joined.trim().to_upper().contains("N") && !parts.is_empty() && evens.contains(2)
    if ok && n == 3 && last == 4 && v == 1 && keys.len() == 1 { 1 } else { 0 }
}
"#,
    );
}

#[test]
fn unknown_collection_method_is_reported() {
    assert_unknown_ident("fn test() { let xs = [1, 2] xs.nope() }", "List.nope");
    assert_unknown_ident("fn test() { \"a\".nope() }", "String.nope");
}

#[test]
fn collection_method_arguments_are_checked() {
    let errors = errors_for("fn test() { let mut xs = [1, 2] xs.push(\"a\") }");
    assert!(errors
        .iter()
        .any(|error| matches!(error, SemanticErrorKind::TypeMismatch { .. })));
    let errors = errors_for("fn test() { \"a\".split() }");
    assert!(!errors.is_empty());
}

#[test]
fn string_plus_number_or_bool_is_allowed_but_not_plus_list() {
    assert_clean("fn test() { let n = 3 let ok = true \"n = \" + n + \", \" + ok + 1.5 }");
    let errors = errors_for("fn test() { \"a\" + [1, 2] }");
    assert!(errors
        .iter()
        .any(|error| matches!(error, SemanticErrorKind::TypeMismatch { .. })));
}

#[test]
fn sort_reverse_slice_find_are_typed() {
    assert_clean(
        r#"
fn test() {
    let xs = [3, 1, 2]
    let a = xs.sort().reverse().slice(1)
    let b = xs.slice(0, 2)
    let f = xs.find(|x| x > 1).unwrap_or(0)
    let s = "hello".slice(1, 3)
    let t: string = s + a.join(",") + b.len() + f
    t
}
"#,
    );
    let errors = errors_for("fn test() { [1, 2].slice(\"a\") }");
    assert!(!errors.is_empty());
    let errors = errors_for("fn test() { [1, 2].slice() }");
    assert!(!errors.is_empty());
}

#[test]
fn unannotated_return_and_param_types_are_inferred() {
    // `double` has no `-> T` and `n` no type: both come from the body / call site.
    let bad = "fn double(n) { n * 2 }\nfn main() { let s: string = double(3) }";
    assert!(!errors_for(bad).is_empty(), "number result must not fit a string");
    let ok = "fn double(n) { n * 2 }\nfn main() { let s: f64 = double(3) }";
    assert!(errors_for(ok).is_empty());
    // Recursion converges.
    let fib = "fn fib(n) { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }\nfn main() { let x = fib(10) }";
    assert!(errors_for(fib).is_empty());
    // Call sites that disagree leave the parameter open instead of erroring.
    let mixed = "fn show(x) { x }\nfn main() { show(1)\nshow(\"a\") }";
    assert!(errors_for(mixed).is_empty());
}
