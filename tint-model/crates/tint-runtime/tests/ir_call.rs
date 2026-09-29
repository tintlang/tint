// Proves the fix to a long-standing, previously-documented stub: the IR
// VM's `Instr::Call` (crates/tint-ir/src/ir_vm.rs) used to always
// `println!("WARNING: Call not implemented, returning unit")` and return
// `Value::Unit`, no matter what was actually being called -- meaning one
// plain `fn` could never call another `fn` when invoked the normal
// top-level way (`TintVM::call_fn`, which routes a logic function
// through a fresh `IrVM` rather than the tree-walking evaluator).
//
// The fix gives `Instr::Call` two things it needed and didn't have:
//   - a way to recover the callee's name from a bare-identifier call
//     site (`resolve_call_name`, reading back the `LoadLocal` that
//     produced the `func` operand);
//   - a real, isolated call frame per nested call (`call_named_function`,
//     via `std::mem::take`/restore of `locals`/`values`/`computed`/
//     `current_instrs`/`dst_index`) -- needed because ValueIds are only
//     unique WITHIN one function's flat instruction list, so running a
//     callee against the caller's own tables would silently corrupt
//     whichever caller ValueId happened to collide with a callee one.
//
// Making `Call` do something real (rather than a harmless `Unit`-always
// stub) surfaced two more pre-existing bugs that had nothing to do with
// dispatch itself, both fixed alongside it:
//   - `is_lazy` (ir_vm.rs) didn't include `Call`, so a recursive call
//     sitting in a match arm that ISN'T taken ran anyway, unconditionally,
//     before `Match` ever decided which arm to use -- `factorial(0)`
//     overflowed the stack, since the recursive arm's `Call` fired at
//     every level regardless of `n`. `Call` needed the same "only compute
//     it if something actually demands the value" treatment `Binary`/
//     `FieldAccess`/etc. already had.
//   - dead-code elimination (ssa_pass.rs) only treated an explicit
//     `Return`'s operand as a liveness root, so a function with NO
//     explicit `return` (`fn square(x) { x * x }`, the ordinary "last
//     expression is the result" shape) had zero roots and its trailing
//     `Binary`/`Unary`/`Const` -- the only thing the function actually
//     computed -- was stripped outright as "dead", long before this Call
//     fix, by an unrelated optimizer pass. `square`, called on its own,
//     silently returned whatever its last SURVIVING instruction happened
//     to be. `dead_code_elimination` now also roots each block's own last
//     instruction, matching the same implicit-return fallback
//     `ir_vm.rs::run_function` already implements.
//
// `examples/factorial.tn` (already in this repo, used as the CLI's own
// smoke example) is exactly this shape -- `sum_of_squares` calls `square`
// twice, `factorial` calls itself, neither has an explicit `return` -- so
// its real source is reused here rather than inventing a new snippet.

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
fn lambdas_work_for_top_level_calls_and_capture_locals() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
fn lambda_math() {
    let base = 10;
    let add = |x| x + base;
    add(5)
}

fn lambda_composition() {
    let add1 = |x| x + 1;
    let twice = |x| add1(add1(x));
    twice(5)
}
"#,
        &mut vm,
    );

    assert_eq!(
        expect_number(
            vm.call_fn("lambda_math", &[], tint_ast::Span::dummy())
                .expect("lambda function should succeed")
        ),
        15.0
    );
    assert_eq!(
        expect_number(
            vm.call_fn("lambda_composition", &[], tint_ast::Span::dummy())
                .expect("nested lambda calls should succeed")
        ),
        7.0
    );
}

#[test]
fn impl_method_can_mutate_self_and_write_back_to_the_receiver() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
struct Point {
    x{i32},
    y{i32},
}

impl Point {
    fn move_by(&mut self, dx) {
        self.x = self.x + dx
    }
}

fn update() {
    let mut point = Point { x{1}, y{2} }
    point.move_by(3)
    point.x
}
"#,
        &mut vm,
    );

    let result = vm
        .call_fn("update", &[], tint_ast::Span::dummy())
        .expect("method call should succeed");
    assert_eq!(expect_number(result), 4.0);
}

#[test]
fn a_fn_with_no_explicit_return_keeps_its_trailing_expression() {
    // Isolates the DCE half of the bug described above from the Call
    // half: `square` has no explicit `return` and calls nothing --
    // before the `ssa_pass.rs` fix, its `Binary { "*", x, x }` was
    // unconditionally stripped as dead code (nothing else in the
    // function referenced its dst), and `square` silently returned
    // whatever its last surviving instruction was instead of `x * x`.
    let mut vm = TintVM::new();
    parse_and_run("fn square(x) { x * x }", &mut vm);

    let result = vm
        .call_fn("square", &[Value::Number(7.0)], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 49.0);
}

#[test]
fn a_plain_fn_can_call_another_plain_fn_via_the_ir_path() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
fn square(x) {
    x * x
}

fn sum_of_squares(a, b) {
    square(a) + square(b)
}
"#,
        &mut vm,
    );

    // `call_fn` (not `call_user_fn`) is the top-level entry point that
    // routes a plain `fn` through the IR VM -- this is the exact call
    // site that used to silently return Unit for `square(a)`/`square(b)`.
    let result = vm
        .call_fn(
            "sum_of_squares",
            &[Value::Number(3.0), Value::Number(4.0)],
            tint_ast::Span::dummy(),
        )
        .expect("call should succeed");

    // 3*3 + 4*4 = 25. Before the fix this was 0 (both `square` calls
    // resolved to Unit, and Unit coerced to 0 in the `+`).
    assert_eq!(expect_number(result), 25.0);
}

#[test]
fn a_recursive_ir_compiled_fn_calls_itself_correctly() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
fn factorial(n) {
    match n {
        0 => 1,
        _ => n * factorial(n - 1),
    }
}
"#,
        &mut vm,
    );

    let result = vm
        .call_fn("factorial", &[Value::Number(5.0)], tint_ast::Span::dummy())
        .expect("call should succeed");

    // 5! = 120. Before the fix, `factorial(n - 1)` resolved to Unit, so
    // the recursive case just returned `n * 0` and the real answer never
    // came up even by accident.
    assert_eq!(expect_number(result), 120.0);
}

#[test]
fn calling_an_undefined_function_reports_unit_instead_of_panicking() {
    let mut vm = TintVM::new();
    parse_and_run(
        r#"
fn go() {
    does_not_exist(1)
}
"#,
        &mut vm,
    );

    // No function named `does_not_exist` exists anywhere in the program
    // (not IR-compiled, not a native, not a builtin) -- `call_named_function`
    // returns `None` for it, and the `Call` handler falls back to `Unit`
    // with a printed warning rather than panicking the whole VM.
    let result = vm
        .call_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed even though the callee doesn't exist");

    assert!(matches!(result, Value::Unit));
}
