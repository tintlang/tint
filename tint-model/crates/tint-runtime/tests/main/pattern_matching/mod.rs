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

include!("matches.rs");
include!("assignments.rs");
