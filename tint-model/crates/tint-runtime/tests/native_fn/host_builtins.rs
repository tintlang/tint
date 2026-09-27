#[test]
fn storage_and_http_are_available_to_tint_source() {
    let mut vm = TintVM::new();
    let code = r#"
        fn go() {
            storage_set("score", "120")
            http_get("/scores")
            storage_get("score")
        }
    "#;
    parse_and_run(code, &mut vm);

    let result = vm
        .call_user_fn("go", &[], tint_ast::Span::dummy())
        .expect("host builtins should run");
    assert!(matches!(result, Value::String(ref value) if value == "120"));
    assert_eq!(vm.storage_snapshot().get("score"), Some(&"120".to_string()));
    assert_eq!(vm.take_http_requests().len(), 1);
    assert_eq!(vm.take_http_requests().len(), 0);
}
