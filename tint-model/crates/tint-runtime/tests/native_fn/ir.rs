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
