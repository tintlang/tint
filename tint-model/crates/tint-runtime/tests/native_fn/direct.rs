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
