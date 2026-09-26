// This used to be one ~1140-line file: a single `#[test] fn
// test_tint_capabilities()` holding one giant Tint source string
// covering ~80 disparate language constructs, followed by a lex ->
// parse -> load -> call sequence whose only real checks were "did the
// parser/loader panic" -- every actual computed result went through
// `println!("... = {:?}", ..)` rather than an assertion, so the test
// couldn't fail on a wrong answer no matter what came out.
//
// Split into `tests/main/` so the two honest halves aren't sharing one
// test function: `parses_and_loads.rs` (does the ~80-construct source
// lex/parse/load without panicking) and `pattern_matching.rs` (real
// `assert_eq!`s, hand-verified against the fixed interpreter, for the
// handful of functions that compute something actually checkable).
// `fixture.rs` holds the shared source text (copied verbatim from the
// original inline string) and a `load_vm()` helper both files call.
mod fixture;
mod parses_and_loads;
mod pattern_matching;
