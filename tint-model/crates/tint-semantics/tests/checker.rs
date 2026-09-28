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
