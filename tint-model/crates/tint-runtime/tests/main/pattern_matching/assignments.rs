#[test]
fn test_array_assign_mutates_the_array_in_place() {
    // `arr[1] = 10; arr[1] + arr[2]` on `[1,2,3]`. Before the fix,
    // `Instr::IndexStore` was a `println!("WARNING: ... not implemented")`
    // no-op: the assignment ran but changed nothing, so this silently
    // read the ORIGINAL `arr[1]` (2) instead of the assigned 10, giving
    // 2+3=5. Fixed value: 10+3=13.
    let mut vm = fixture::load_vm();
    let result = vm
        .call_fn("TestArrayAssign", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 13.0);
}

#[test]
fn test_struct_field_assign_mutates_the_struct_in_place() {
    // `u.age = 20; u.age` -- same class of bug as the array case above,
    // for `Instr::FieldStore` instead of `IndexStore`. Used to silently
    // read back the original `age` (10) instead of the assigned 20.
    let mut vm = fixture::load_vm();
    let result = vm
        .call_fn("TestStructFieldAssign", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 20.0);
}

#[test]
fn test_map_assign_inserts_or_overwrites_the_key() {
    // `m.b = 5; m.b * 2` on `map { a{1}, b{2} }`. Also exercises that
    // `FieldStore`'s fix handles `Value::Map` (insert-or-overwrite),
    // not just `Value::StructInstance` (existing-field-only) -- these
    // have different semantics on a MISSING key (a struct field store
    // only ever targets an already-declared field; a map key is created
    // on first assignment), mirrored from the tree-walking evaluator's
    // `Value::set_field`.
    let mut vm = fixture::load_vm();
    let result = vm
        .call_fn("TestMapAssign", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 10.0); // 5*2
}

#[test]
fn test_struct_update_field_builds_a_new_struct_without_mutating_the_original() {
    // `User { age{u.age + 1}, ..u }` -- a SPREAD-update EXPRESSION
    // (`Instr::StructUpdate`), not an in-place assignment: it builds a
    // NEW struct value from `u` with `age` overridden, and `u` itself is
    // untouched. Used to be a `println!` stub that always returned
    // `Unit` regardless of `base`/`updates`.
    let mut vm = fixture::load_vm();
    let result = vm
        .call_fn("TestStructUpdateField", &[], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 11.0); // u.age(10) + 1
}
