// Real, hand-verified assertions for the handful of functions in the
// shared fixture (`fixture.rs`) whose result is actually checkable --
// upgraded from the original single test's `println!("... = {:?}", …)`,
// which never failed no matter what came out.
//
// `AllFeatures` took real digging to get right: a naive hand-trace of its
// source gives 1334, and that's genuinely correct now, but it took three
// separate interpreter bugs -- found one at a time by chasing why the
// runtime kept disagreeing with the hand-trace -- before it was true. See
// the long comment on that test for exactly what each one was and where
// it was fixed.

use super::fixture;
use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;

fn expect_number(v: Value) -> f64 {
    match v {
        Value::Number(n) => n,
        other => panic!("expected a Number, got {:?}", other),
    }
}

fn expect_string(v: Value) -> String {
    match v {
        Value::String(s) => s,
        other => panic!("expected a String, got {:?}", other),
    }
}

#[test]
fn all_features_computes_the_documented_value() {
    // `AllFeatures` (see `fixture.rs`'s `SOURCE`) computes
    // `s1 + a0 + a3 + r1 + r2 + r3 + who`. Traced by hand:
    //   s1  = x+y+z with (x,y,z)=(10,20,30)          = 60
    //   a0  = arr[0], arr=[x,y,z,s1]                  = 10
    //   a3  = arr[3]                                  = 60
    //   r1  = match en { A{x}=>x*2, .. }, en=A{x{50}} = 100
    //   r2  = match tval(42) { x: i32 => x*2, .. }    = 84
    //   r3  = match (1,2,3) { (1,b,3) => b*10, .. }   = 20
    //   who = match user2 { User{id,name}=>id*100 }   = 1000
    // 60+10+60+100+84+20+1000 = 1334.
    //
    // Getting the interpreter to actually PRODUCE 1334 (it used to return
    // 1170) took three separate bug fixes, found one at a time by
    // chasing why the runtime kept disagreeing with this trace:
    //
    //   1. r2 (the typed-pattern arm) -- two bugs stacked here:
    //      - `ir_vm.rs`'s `pattern_matches` had no `Pattern::Typed` case
    //        at all -- it fell into the catch-all `_ => true`, matching
    //        unconditionally with NO binding, unlike the tree-walking
    //        evaluator's already-correct `matches_type` + bind. Fixed to
    //        mirror that.
    //      - Separately, and this is what actually made r2 wrong even
    //        after that fix: `tint-ir`'s compiler lowers one whole
    //        function into a single flat instruction list with ONE
    //        shared compile-time name -> ValueId cache, and match arms
    //        never touched that cache for their own pattern bindings.
    //        `tval`'s arm rebinds `x`, already in the cache from the
    //        earlier `let (x, y, z) = p;` destructure -- so `x * 2` in
    //        the arm body resolved straight to the OUTER `x`'s
    //        already-computed ValueId at compile time and never even
    //        became a `LoadLocal` the runtime binding could feed. Fixed
    //        in `compiler.rs`'s `Expr::Match` lowering: a pattern's
    //        bound names are now shadowed out of that cache (and
    //        restored after) before the arm body is lowered, forcing a
    //        fresh `LoadLocal` that actually reads back what
    //        `Instr::Match` bound at runtime.
    //
    //   2. r1 (the struct-style variant arm) -- `en` is
    //      `E2::A { x{50} }`, a struct-LIKE enum variant (named field,
    //      not `A(x)`-style positional). `A { x } => ..` parses to
    //      `Pattern::Struct` (the parser can't tell, at a bare `Name {`
    //      pattern, whether `Name` is a struct or an enum variant), which
    //      used to only ever match `Value::StructInstance`, never
    //      `Value::EnumInstance` -- so this always fell through to
    //      `_ => 0` regardless of `en`'s actual variant. Fixed by letting
    //      `Pattern::Struct` accept either value kind (struct by struct
    //      name, enum by variant name), which needed
    //      `Value::EnumInstance` to retain field NAMES instead of a
    //      positional-only `Vec<Value>` -- see its doc comment in
    //      tint-ir/src/ir.rs. Scoped to this IR VM's own `Value`; the
    //      tree-walking evaluator has its own separate `Value` type and
    //      is untouched.
    //
    // (TestGuard, below, is the third bug this pass found and fixed --
    // match guards weren't evaluated at all.)
    let mut vm = fixture::load_vm();
    let result = vm
        .call_fn("AllFeatures", &[], tint_ast::Span::dummy())
        .expect("AllFeatures should run");

    assert_eq!(expect_number(result), 1334.0);
}

#[test]
fn test_match_numbers_matches_the_zero_and_fallthrough_arms() {
    let mut vm = fixture::load_vm();

    let zero = vm
        .call_fn("TestMatchNumbers", &[Value::Number(0.0)], tint_ast::Span::dummy())
        .expect("call should succeed");
    assert_eq!(expect_string(zero), "zero");

    let other = vm
        .call_fn("TestMatchNumbers", &[Value::Number(5.0)], tint_ast::Span::dummy())
        .expect("call should succeed");
    assert_eq!(expect_string(other), "other");
}

#[test]
fn test_match_tuple_binds_both_elements_in_the_fallthrough_arm() {
    // `(0,0) => 0` doesn't match (3,4), so this falls to `(x,y) => x*x+y*y`.
    let mut vm = fixture::load_vm();
    let point = Value::Tuple(vec![Value::Number(3.0), Value::Number(4.0)]);

    let result = vm
        .call_fn("TestMatchTuple", &[point], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_number(result), 25.0); // 3*3 + 4*4
}

#[test]
fn test_match_struct_binds_fields_and_interpolates_them() {
    let mut vm = fixture::load_vm();
    let user = Value::StructInstance {
        name: "User".into(),
        fields: vec![
            ("id".into(), Value::Number(7.0)),
            ("name".into(), Value::String("A".into())),
            ("age".into(), Value::Number(30.0)),
        ],
    };

    let result = vm
        .call_fn("TestMatchStruct", &[user], tint_ast::Span::dummy())
        .expect("call should succeed");

    assert_eq!(expect_string(result), "7:A");
}

#[test]
fn test_guard_actually_evaluates_the_condition() {
    // This used to be unable to fail: `Instr::Match` (ir_vm.rs) had
    // `let guard_passes = if let Some(_guard_id) = guard { true } else {
    // true }` -- a hardcoded `// TODO: evaluate guard expression` that
    // treated every guard as unconditionally true. `TestGuard(50)`
    // happened to look right (its one true arm's condition, `x > 10`, is
    // genuinely true for 50), but the same source would ALSO have
    // returned "big" for `TestGuard(5)` -- the guard had literally no
    // effect on which arm ran. Fixed by having `Instr::Match` actually
    // demand the guard expression's value (after installing the
    // pattern's bindings, since the guard references them) instead of
    // skipping straight past it; this needed `Instr::Binary` to gain
    // comparison/boolean operators (`<`, `>`, `<=`, `>=`, `==`, `!=`,
    // `&&`, `||`) it was also missing entirely, mirroring the
    // tree-walking evaluator's already-correct `Expr::Binary` handling
    // op-for-op. Asserting BOTH outcomes here (not just the one call the
    // original fixture happened to make) is what actually proves the
    // guard is doing something now, rather than merely returning the
    // same answer it always did.
    let mut vm = fixture::load_vm();

    let big = vm
        .call_fn("TestGuard", &[Value::Number(50.0)], tint_ast::Span::dummy())
        .expect("call should succeed");
    assert_eq!(expect_string(big), "big");

    let small = vm
        .call_fn("TestGuard", &[Value::Number(5.0)], tint_ast::Span::dummy())
        .expect("call should succeed");
    assert_eq!(expect_string(small), "small");
}

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
