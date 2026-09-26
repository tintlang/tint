// Proves `TintVM::register_native` -- direct Rust interop: register an
// ordinary Rust closure under a name, and `.tn` source calls it directly, no
// Rust parsing or borrow-checking of Tint code involved anywhere.
//
// Three call sites are exercised, matching the three places this is
// actually reachable in the current runtime:
//   - `TintVM::call_fn`, the direct/top-level invocation an embedder uses
//     (this is exactly what `tint run file.tn <name>` calls with `<name>`
//     as `fn_name` -- if `<name>` is a registered native rather than a
//     Tint `fn`, it's called with no Tint source involved at all).
//   - `TintVM::call_user_fn`, the tree-walking dispatch used for
//     click/hover handlers (see `UiSession::dispatch`) and other
//     tree-walked bodies -- a *nested* call from inside a Tint `fn`'s body
//     to a native fn now resolves correctly there (see the `Expr::Call`
//     fix in tint-evaluator/src/evaluator/expr.rs).
//   - `TintVM::call_fn`'s IR branch: a plain `fn`, invoked the normal
//     top-level way (so it runs through `IrVM`, not the tree-walking
//     evaluator), whose OWN body calls a native fn. This used to be a real
//     gap: `tint-ir` (where `IrVM` lives) has no dependency on
//     `tint-runtime`'s `native_fns` table at all, so a name that resolved
//     to nothing among the IR-compiled `.tn` functions just returned
//     `None` and the call silently became `Unit`, no matter what was
//     registered. Fixed by giving `IrVM` a `native_call` callback slot
//     (`set_native_call`) that `call_named_function` falls back to when no
//     IR-compiled function matches the name, and having `call_fn`'s IR
//     branch install one backed by `TintVM::native_fns` before running the
//     `IrVM`. `native_fns` had to move from a plain `HashMap` to
//     `Rc<RefCell<HashMap<...>>>` so that callback -- which must be
//     `'static`, since `IrVM` holds it with no lifetime parameter -- could
//     `Rc::clone` a handle into the SAME table instead of borrowing `&self`
//     (which would fight the `&mut self` receiver of `call_fn`). See
//     `native_fn_reachable_from_a_plain_fn_called_via_the_ir_path` below.

use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn parse_and_run(code: &str, vm: &mut TintVM) {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
}

#[test]
fn native_fn_called_directly_with_no_tint_source_involved() {
    let mut vm = TintVM::new();

    // A real Rust closure -- ordinary Rust, nothing Tint-specific about the
    // computation itself. Doubling stands in for "call into any real
    // Rust/crate code you want".
    vm.register_native("double", |args| {
        let n = match args.get(0) {
            Some(Value::Number(n)) => *n,
            _ => 0.0,
        };
        Ok(Value::Number(n * 2.0))
    });

    // No `.tn` source at all: this is the `tint run file.tn double` shape,
    // where the CLI's entry-point name happens to be a native instead of a
    // Tint `fn`.
    let result = vm
        .call_fn("double", &[Value::Number(21.0)], tint_ast::Span::dummy())
        .expect("call should succeed");

    match result {
        Value::Number(n) => assert_eq!(n, 42.0),
        other => panic!("expected Number(42), got {:?}", other),
    }
}

#[test]
fn native_fn_reachable_from_inside_a_tree_walked_fn_body() {
    let mut vm = TintVM::new();
    vm.register_native("shout", |args| {
        let s = match args.get(0) {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        Ok(Value::String(s.to_uppercase()))
    });

    let code = r#"
        fn go() {
            shout("hi")
        }
    "#;
    parse_and_run(code, &mut vm);

    // `call_user_fn` tree-walks `go`'s body directly -- this is the same
    // dispatch a click/hover handler goes through (`UiSession::dispatch`),
    // unlike `call_fn`, which would route a plain `fn` through the IR VM
    // instead (see the module doc comment above).
    let result = vm
        .call_user_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    match result {
        Value::String(s) => assert_eq!(s, "HI"),
        other => panic!("expected String(\"HI\"), got {:?}", other),
    }
}

#[test]
fn native_fn_shadows_a_builtin_on_purpose() {
    let mut vm = TintVM::new();
    // "sqrt" already exists as a hardcoded builtin (see
    // tint-evaluator/src/utils/call.rs) -- registering a native under the
    // same name should win, proving native_fns is checked first.
    vm.register_native("sqrt", |_args| Ok(Value::Number(-1.0)));

    let code = r#"
        fn go() {
            sqrt(9)
        }
    "#;
    parse_and_run(code, &mut vm);

    let result = vm
        .call_user_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    match result {
        Value::Number(n) => assert_eq!(n, -1.0),
        other => panic!("expected the native override's Number(-1), got {:?}", other),
    }
}

#[test]
fn native_fn_reachable_from_a_plain_fn_called_via_the_ir_path() {
    let mut vm = TintVM::new();
    vm.register_native("shout", |args| {
        let s = match args.get(0) {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        Ok(Value::String(s.to_uppercase()))
    });

    let code = r#"
        fn go() {
            shout("hi")
        }
    "#;
    parse_and_run(code, &mut vm);

    // The key difference from `native_fn_reachable_from_inside_a_tree_walked_fn_body`
    // above: `call_fn` (not `call_user_fn`) is the normal top-level entry
    // point, and for a plain `fn` it routes through the IR VM
    // (`TintVM::is_ir_function` + `IrVM::run_with_args`), not the
    // tree-walking evaluator. Before this fix, the IR VM had no way to
    // reach `native_fns` at all and this would have returned `Unit`
    // instead of the native's actual result.
    let result = vm
        .call_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    match result {
        Value::String(s) => assert_eq!(s, "HI"),
        other => panic!("expected String(\"HI\"), got {:?}", other),
    }
}

#[test]
fn native_fn_called_from_one_ir_fn_via_another_still_reaches_it() {
    // Same gap, one level of indirection further: `go` (IR-compiled) calls
    // `helper` (also IR-compiled), whose own body calls the native. Proves
    // the native-call fallback in `call_named_function` fires on a nested
    // IR-to-IR call too, not just when the top-level `call_fn` target
    // itself contains the native call.
    let mut vm = TintVM::new();
    vm.register_native("double", |args| {
        let n = match args.get(0) {
            Some(Value::Number(n)) => *n,
            _ => 0.0,
        };
        Ok(Value::Number(n * 2.0))
    });

    let code = r#"
        fn helper(n) {
            double(n)
        }

        fn go() {
            helper(21)
        }
    "#;
    parse_and_run(code, &mut vm);

    let result = vm
        .call_fn("go", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    match result {
        Value::Number(n) => assert_eq!(n, 42.0),
        other => panic!("expected Number(42), got {:?}", other),
    }
}
