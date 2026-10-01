//! Scalar programs run on the wasm backend and on the reference interpreter;
//! the results (or trap messages) must be identical.

mod common;
use common::run;
use tint_ir::typed::interp::render;
use tint_ir::typed::{lower_program, Interp, Module};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn lower(source: &str) -> Module {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    assert!(errors.is_empty(), "{errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
    lowered.module
}

fn reference(m: &Module, entry: &str) -> Result<String, String> {
    let mut interp = Interp::new(m);
    interp.init().map_err(|t| t.msg)?;
    match interp.run(entry, vec![]) {
        Ok((v, ty)) => Ok(render(m, ty, &v)),
        Err(t) => Err(t.msg),
    }
}

/// The messages differ in detail (the interpreter prints the offending value);
/// compare the part up to the first colon.
fn same(a: &Result<String, String>, b: &Result<String, String>) -> bool {
    match (a, b) {
        (Ok(x), Ok(y)) => x == y,
        (Err(x), Err(y)) => x.split(':').next() == y.split(':').next(),
        _ => false,
    }
}

fn check(source: &str) {
    let m = lower(source);
    let mut entries: Vec<&String> = m.functions.keys().filter(|n| n.starts_with("t_")).collect();
    entries.sort();
    assert!(!entries.is_empty());
    let mut bad = Vec::new();
    for e in entries {
        let want = reference(&m, e);
        match run(&m, e) {
            Err(why) => bad.push(format!("{e}: {why}")),
            Ok(got) => {
                if !same(&want, &got) {
                    bad.push(format!("{e}: interpreter {want:?}, wasm {got:?}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn control_flow() {
    check(
        r#"
fn t_while() { let mut i = 0; let mut s = 0; while i < 10 { s = s + i; i = i + 1 }; s }
fn t_for() { let mut s = 0; for i in 0..100 { s = s + i * i }; s }
fn t_break() { let mut s = 0; for i in 0..100 { if i == 7 { break }; s = s + i }; s }
fn t_continue() { let mut s = 0; for i in 0..20 { if i % 3 == 0 { continue }; s = s + i }; s }
fn t_nested() {
    let mut s = 0
    for i in 0..10 { for j in 0..10 { if j > i { break }; s = s + i * j } }
    s
}
fn t_nested_continue() {
    let mut s = 0
    for i in 0..10 { for j in 0..10 { if (i + j) % 2 == 0 { continue }; s = s + 1 } }
    s
}
fn t_loop() { let mut i = 0; loop { i = i + 1; if i == 50 { break } }; i }
fn t_return_in_loop() { let mut i = 0; while true { if i == 13 { return i }; i = i + 1 }; 99 }
fn fib(n) { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
fn t_fib() { fib(20) }
fn collatz(n) { let mut x = n; let mut steps = 0; while x != 1 { if x % 2 == 0 { x = x / 2 } else { x = 3 * x + 1 }; steps = steps + 1 }; steps }
fn t_collatz() { collatz(27) }
fn t_and_or() { let a = 3; let b = 5; if a < b && b < 10 || a > 100 { 1 } else { 0 } }
fn t_not() { let a = true; if !a { 1 } else { 2 } }
fn t_match() {
    let mut s = 0
    for i in 0..8 { s = s + match i { 0 => 10, 1 => 20, n if n > 5 => 1000, _ => 1 } }
    s
}
"#,
    );
}

#[test]
fn integer_arithmetic_and_traps() {
    check(
        r#"
fn t_i32_add() -> i32 { let a: i32 = 2147483647; a + 1 }
fn t_i32_ok() -> i32 { let a: i32 = 2147483646; a + 1 }
fn t_i32_mul() -> i32 { let a: i32 = 65536; a * a }
fn t_i32_neg_min() -> i32 { let a: i32 = -2147483647; let b = a - 1; -b }
fn t_i32_div_zero() -> i32 { let a: i32 = 5; let z: i32 = 0; a / z }
fn t_i32_rem() -> i32 { let a: i32 = -7; let b: i32 = 3; a % b }
fn t_i32_div() -> i32 { let a: i32 = -7; let b: i32 = 2; a / b }
fn t_u8_over() -> u8 { let a: u8 = 250; a + 10 }
fn t_u8_under() -> u8 { let a: u8 = 3; a - 4 }
fn t_u8_ok() -> u8 { let a: u8 = 250; a + 5 }
fn t_u32_mul() -> u32 { let a: u32 = 4294967295; let b: u32 = 2; a * b }
fn t_u32_max_sq() -> u32 { let a: u32 = 4294967295; a * a }
fn t_i64_add() -> i64 { let a: i64 = 9223372036854775807; a + 1 }
fn t_i64_sub() -> i64 { let a: i64 = -9223372036854775807; let b: i64 = 2; a - b }
fn t_i64_mul_ok() -> i64 { let a: i64 = 3037000499; a * a }
fn t_i64_mul_over() -> i64 { let a: i64 = 3037000500; a * a }
fn t_i64_mul_big() -> i64 { let a: i64 = 4611686018427387904; let b: i64 = 2; a * b }
fn t_i64_mul_neg() -> i64 { let a: i64 = -4611686018427387904; let b: i64 = 2; a * b }
fn t_i64_mul_neg_over() -> i64 { let a: i64 = -4611686018427387905; let b: i64 = 2; a * b }
fn t_i64_mul_m1() -> i64 { let a: i64 = -1; let b: i64 = -9223372036854775807; a * b }
fn t_i64_div_min() -> i64 { let a: i64 = -9223372036854775807; let b = a - 1; let m: i64 = -1; b / m }
fn t_i64_rem() -> i64 { let a: i64 = -100; let b: i64 = 7; a % b }
fn t_u64_add() -> u64 { let a: u64 = 18446744073709551615; a + 1 }
fn t_u64_add_ok() -> u64 { let a: u64 = 18446744073709551614; a + 1 }
fn t_u64_sub() -> u64 { let a: u64 = 1; a - 2 }
fn t_u64_mul() -> u64 { let a: u64 = 4294967296; a * a }
fn t_u64_mul_ok() -> u64 { let a: u64 = 4294967295; a * a }
fn t_u64_mul_big() -> u64 { let a: u64 = 9223372036854775808; a * 2 }
fn t_u64_div() -> u64 { let a: u64 = 18446744073709551615; a / 3 }
fn t_u64_rem() -> u64 { let a: u64 = 18446744073709551615; a % 10 }
fn t_u64_cmp() -> bool { let a: u64 = 18446744073709551615; let b: u64 = 1; a > b }
fn t_neg_u() -> u32 { let a: u32 = 5; -a }
"#,
    );
}

#[test]
fn floats_and_casts() {
    check(
        r#"
fn t_add() { 0.1 + 0.2 }
fn t_div() { 1 / 3 }
fn t_div0() { let z = 0; 1 / z }
fn t_rem() { 7.5 % 2 }
fn t_rem_neg() { -7.5 % 2 }
fn t_neg() { let x = 3.5; -x }
fn t_f32() -> f32 { let a: f32 = 0.1; let b: f32 = 0.2; a + b }
fn t_f32_mul() -> f32 { let a: f32 = 16777216.0; a * 3.3 }
fn t_cast_i32() -> i32 { 3.0 as i32 }
fn t_cast_frac() -> i32 { 3.9 as i32 }
fn t_cast_neg_i32() -> i32 { -3.0 as i32 }
fn t_cast_big() -> i32 { 3000000000.0 as i32 }
fn t_cast_u8() -> u8 { 255.0 as u8 }
fn t_cast_u8_big() -> u8 { 256.0 as u8 }
fn t_cast_u8_neg() -> u8 { -1.0 as u8 }
fn t_cast_u32() -> u32 { 4294967295.0 as u32 }
fn t_cast_u32_big() -> u32 { 4294967296.0 as u32 }
fn t_cast_i64() -> i64 { 4611686018427387904.0 as i64 }
fn t_cast_i64_edge() -> i64 { 9223372036854775808.0 as i64 }
fn t_cast_u64() -> u64 { 9223372036854775808.0 as u64 }
fn t_cast_u64_edge() -> u64 { 18446744073709551616.0 as u64 }
fn t_cast_nan() -> i32 { let z = 0; let n = z / z; n as i32 }
fn t_int_to_float() { let a: i32 = 7; a as number }
fn t_u64_to_float() { let a: u64 = 18446744073709551615; a as f64 }
fn t_i64_to_f32() -> f32 { let a: i64 = 16777217; a as f32 }
fn t_f64_to_f32() -> f32 { let a = 1.0000000001; a as f32 }
fn t_f64_to_f32_big() -> f32 { let a = 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0 * 1000000000000000.0; a as f32 }
fn t_i64_to_i32() -> i32 { let a: i64 = 2147483647; a as i32 }
fn t_i64_to_i32_over() -> i32 { let a: i64 = 2147483648; a as i32 }
fn t_i64_to_i32_under() -> i32 { let a: i64 = -2147483649; a as i32 }
fn t_i64_to_u32() -> u32 { let a: i64 = -1; a as u32 }
fn t_i64_to_u64() -> u64 { let a: i64 = -1; a as u64 }
fn t_i64_to_u64_ok() -> u64 { let a: i64 = 9223372036854775807; a as u64 }
fn t_u64_to_i64() -> i64 { let a: u64 = 9223372036854775808; a as i64 }
fn t_u64_to_i64_ok() -> i64 { let a: u64 = 9223372036854775807; a as i64 }
fn t_u64_to_u32() -> u32 { let a: u64 = 4294967296; a as u32 }
fn t_u32_to_i32() -> i32 { let a: u32 = 2147483648; a as i32 }
fn t_u32_to_i32_ok() -> i32 { let a: u32 = 2147483647; a as i32 }
fn t_i32_to_u8() -> u8 { let a: i32 = 255; a as u8 }
fn t_i32_to_u8_over() -> u8 { let a: i32 = 256; a as u8 }
fn t_u8_to_i32() -> i32 { let a: u8 = 200; a as i32 }
fn t_cmp() { let a = 1.5; let b = 2.5; if a < b && a != b && !(a >= b) { 1 } else { 0 } }
"#,
    );
}

#[test]
fn globals_and_calls() {
    check(
        r#"
const BASE: i64 = 40
let counter = 7
fn bump() { counter + 1 }
fn t_const() -> i64 { BASE + 2 }
fn t_global() { bump(); bump(); bump() }
fn add3(a, b, c) { a + b + c }
fn t_args() { add3(1, 2, 3) }
fn t_unit() { let x = 1; x }
fn is_even(n) { if n == 0 { true } else { is_odd(n - 1) } }
fn is_odd(n) { if n == 0 { false } else { is_even(n - 1) } }
fn t_mutual() { is_even(10) }
fn t_bool_eq() { let a = true; let b = false; a == b }
"#,
    );
}
