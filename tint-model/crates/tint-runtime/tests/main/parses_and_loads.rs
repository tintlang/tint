// Honest smoke coverage for the ~80-construct fixture source (see
// `fixture.rs`'s doc comment for why this file exists separately from
// `pattern_matching.rs`, which asserts real computed values for the
// handful of functions where that's actually meaningful).
//
// Most of what's in the shared source -- GPU kernels, borrow blocks,
// several of the generic-fn shapes, UI parsing -- doesn't have real
// runtime semantics wired up yet (some print a "not implemented"
// warning and return `Unit`, some are accepted by the parser purely as
// forward-looking syntax). Calling any of those and asserting on the
// result would mean asserting on a stand-in value, which is worse than
// not asserting at all -- it reads as verified when it isn't. What IS a
// real, honest claim about that code: it lexes, it parses, and it loads
// into a VM without the loader itself panicking. That's what this test
// checks, and nothing more.

use super::fixture;

#[test]
fn the_full_fixture_source_lexes_parses_and_loads_without_panicking() {
    // `fixture::load_vm` already panics (with a source-span diagnostic
    // for a parse failure, or a plain message for a load-time panic) if
    // any stage fails -- reaching this point at all is the assertion.
    let _vm = fixture::load_vm();
}
