//! Type inference: lambdas, `Option`/`Result` holes, unannotated parameters,
//! and strict mode.

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::errors::SemanticErrorKind;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::{SemanticChecker, SemanticModel, Type};

fn check(source: &str, strict: bool) -> (Vec<SemanticErrorKind>, SemanticModel) {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens)
        .parse_program()
        .unwrap_or_else(|e| panic!("source failed to parse: {:?}\n---\n{}", e, source));
    let ctx = CheckerContext { strict, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    (errors.into_iter().map(|e| e.kind).collect(), model)
}

/// Type of the widest expression that starts where `text` starts (the parser
/// gives some nodes short spans, e.g. an array literal covers only its `[`).
fn type_of_text(source: &str, model: &SemanticModel, text: &str) -> Type {
    let start = source.find(text).unwrap_or_else(|| panic!("`{text}` not in source"));
    model
        .expressions
        .iter()
        .filter(|e| e.span.start.offset == start)
        .max_by_key(|e| e.span.end.offset)
        .unwrap_or_else(|| panic!("no expression at `{text}`"))
        .ty
        .clone()
}

fn generic(name: &str, args: Vec<Type>) -> Type {
    Type::Generic(name.into(), args)
}

#[test]
fn lambda_parameters_take_their_type_from_the_call() {
    let source = r#"
fn test() {
    let words = ["a", "bb"]
    let long = words.filter(|w| w.len() > 1)
    let sizes = words.map(|w| w.len())
    sizes
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(type_of_text(source, &model, "w.len() > 1"), Type::Bool);
    assert_eq!(type_of_text(source, &model, "words.map(|w| w.len())"), Type::Array(Box::new(Type::Number)));
    assert!(model.is_fully_typed());
}

#[test]
fn lambda_parameters_are_learned_from_their_use() {
    let source = "fn test() { let add = |a, b| a + b\n add(1, 2) }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        type_of_text(source, &model, "|a, b| a + b"),
        Type::Fn(Box::new(Type::Number), vec![Type::Number, Type::Number])
    );
}

#[test]
fn function_valued_parameters_are_inferred_from_call_sites() {
    let source = r#"
fn square(x) { x * x }
fn apply(f, x) { f(x) }
fn main() {
    let a = apply(square, 3)
    let b = apply(|x| x * 4, 3)
    a + b
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(model.is_fully_typed());
    let (params, ret) = &model.functions["apply"];
    assert_eq!(ret, &Type::Number);
    assert_eq!(params[1], Type::Number);
    assert_eq!(params[0], Type::Fn(Box::new(Type::Number), vec![Type::Number]));
}

#[test]
fn option_and_result_holes_are_filled_from_context() {
    let source = r#"
fn none() -> Option<i32> {
    Option::None {}
}
fn ok() -> Result<number, string> {
    Result::Ok { value: 1 }
}
fn err() -> Result<number, string> {
    Result::Err { error: "bad" }
}
fn main() {
    let a = none()
    let b = ok()
    let c = err()
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(type_of_text(source, &model, "None {}"), generic("Option", vec![Type::Simple("i32".into())]));
    assert_eq!(
        type_of_text(source, &model, "Ok { value: 1 }"),
        generic("Result", vec![Type::Number, Type::String])
    );
    assert_eq!(
        type_of_text(source, &model, "Err { error: \"bad\" }"),
        generic("Result", vec![Type::Number, Type::String])
    );
}

#[test]
fn an_unconstrained_hole_defaults_to_unit_instead_of_staying_unknown() {
    let source = r#"
fn main() {
    let xs = []
    let n = Option::None {}
    xs.len()
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(type_of_text(source, &model, "[]"), Type::Array(Box::new(Type::Unit)));
    assert_eq!(type_of_text(source, &model, "None {}"), generic("Option", vec![Type::Unit]));
}

#[test]
fn match_patterns_bind_payloads_with_their_types() {
    let source = r#"
fn describe(o: Option<string>) -> number {
    match o {
        Some { value } => value.len(),
        None => 0,
    }
}
fn total(r: Result<number, string>) -> number {
    match r {
        Ok { value } => value + 1,
        Err { error } => error.len(),
    }
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(type_of_text(source, &model, "value.len()"), Type::Number);
    assert_eq!(type_of_text(source, &model, "error.len()"), Type::Number);
}

#[test]
fn a_pattern_can_pin_down_an_unannotated_parameter() {
    let source = r#"
enum Shape { Circle { r }, Square { side }, Empty }
fn area(shape) {
    match shape {
        Circle { r } => r * r * 3,
        Square { side } => side * side,
        _ => 0,
    }
}
fn main() { area(Shape::Circle { r: 2 }) }
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["area"].0[0], Type::Enum("Shape".into()));
    assert_eq!(model.functions["area"].1, Type::Number);
}

#[test]
fn methods_on_a_parameter_wait_for_the_call_site() {
    // `xs.len()` is checked before any caller is seen.
    let source = "fn count(xs) { xs.len() }\nfn main() { count([1, 2]) }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["count"].0[0], Type::Array(Box::new(Type::Number)));
    assert_eq!(model.functions["count"].1, Type::Number);
}

#[test]
fn builtins_that_only_print_return_unit() {
    let source = "fn main() { println(\"hi\") }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["main"].1, Type::Unit);
    assert_eq!(type_of_text(source, &model, "println(\"hi\")"), Type::Unit);
}

#[test]
fn strict_mode_reports_what_cannot_be_inferred() {
    // Nothing ever calls `f`, so `x` has no type.
    let source = "fn f(x) { x }";
    let (errors, _) = check(source, false);
    assert!(errors.is_empty(), "{errors:?}");
    let (errors, _) = check(source, true);
    assert!(
        errors.iter().any(|e| matches!(e, SemanticErrorKind::CannotInfer(what) if what.contains("parameter `x` of `f`"))),
        "{errors:?}"
    );
}

#[test]
fn strict_mode_asks_for_annotations_on_parameters_used_at_two_types() {
    let source = "fn show(x) { x }\nfn main() { show(1)\nshow(\"a\") }";
    let (errors, model) = check(source, false);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["show"].0[0], Type::Unknown);
    let (errors, _) = check(source, true);
    assert!(
        errors.iter().any(|e| matches!(e, SemanticErrorKind::CannotInfer(what) if what.contains("annotate"))),
        "{errors:?}"
    );
}

#[test]
fn sized_numeric_types_flow_through_arithmetic() {
    let source = "fn test(a: i32, b: i32) -> i32 { a + b * 2 }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(type_of_text(source, &model, "a + b * 2"), Type::Simple("i32".into()));
}

#[test]
fn a_literal_only_argument_does_not_hide_a_sized_call_site() {
    let source = "fn double(n) { n * 2 }\nfn main() { let a: i32 = 4\n double(a) }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["double"].0[0], Type::Simple("i32".into()));
    assert_eq!(model.functions["double"].1, Type::Simple("i32".into()));
}

#[test]
fn every_expression_of_the_model_can_be_looked_up_by_node() {
    let source = "fn test() { 1 + 2 }";
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().unwrap();
    let (errors, model) = SemanticChecker::new(CheckerContext::default()).check_with_model(&program);
    assert!(errors.is_empty());
    let tint_ast::Item::Fn(function) = &program.items[0] else { panic!() };
    let tint_ast::FnBody::Block(block) = &function.body else { panic!() };
    let tint_ast::Stmt::Expr(expr) = &block.stmts[0] else { panic!() };
    assert_eq!(model.type_of(expr), Some(&Type::Number));
}

#[test]
fn shadowing_a_let_in_the_same_scope_is_allowed() {
    let (errors, _) = check("fn test() { let x = 1\n let x = x + 2\n x }", true);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn a_loop_that_only_returns_does_not_fall_off_the_end() {
    let source = "fn main() { loop { return 0 } }";
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(model.functions["main"].1, Type::Number);
}

#[test]
fn option_and_result_methods_are_fully_typed() {
    let source = r#"
fn a() {
    let o: Option<number> = Some(3)
    let b = o.is_none()
    let c = o.map(|x| x + 1)
    let d = o.and_then(|x| Some(x * 2))
    let e = o.ok_or("none")
    let f = o.filter(|x| x > 1)
    let g = o.or(Some(1))
    let h = o.is_some_and(|x| x > 1)
    let r: Result<number, string> = Ok(1)
    let i = r.map_err(|e| e + "!")
    let j = r.ok()
    let k = r.err()
    let l = r.unwrap_err()
    let m = r.unwrap_or_else(|e| 0)
    let n = r.or_else(|e| Ok(0))
    let p = r.map_or(0, |x| x * 2)
    b
}
"#;
    let (errors, model) = check(source, true);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(model.expressions.iter().all(|t| !format!("{:?}", t.ty).contains("Unknown")));
}

#[test]
fn option_method_on_result_and_bare_option_are_errors() {
    let (errors, _) = check("fn a() { let r: Result<number, string> = Ok(1)\n r.is_some() }", false);
    assert!(!errors.is_empty());
    let (errors, _) = check("fn b(o: Option) -> number { 1 }", false);
    assert!(!errors.is_empty());
}
