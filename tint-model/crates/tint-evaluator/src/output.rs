// `print()`/`dbg()` (see utils/call.rs) used to go straight to real
// stdout via `println!`, which is fine for the CLI but writes nowhere
// useful when the evaluator runs inside a browser (tint-wasm has no real
// stdout to write to). This is a small captured-output buffer: every
// print/dbg call appends a line here (in addition to `println!`, so the
// CLI/terminal experience is unchanged), and a host that needs to show
// the program's output -- currently tint-wasm's `run()`/`render_ui()` --
// calls `take_output()` once per run to drain it.
//
// Thread-local rather than global/shared: each native test or wasm call
// gets its own buffer, so concurrent runs (e.g. the test suite) never
// see each other's output.

use std::cell::RefCell;

thread_local! {
    static OUTPUT: RefCell<String> = RefCell::new(String::new());
}

/// Appends one line to the current thread's captured output.
pub fn write_line(line: &str) {
    OUTPUT.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.push_str(line);
        buf.push('\n');
    });
}

/// Returns everything captured since the last `take_output()` call (or
/// since the thread started), and clears the buffer.
pub fn take_output() -> String {
    OUTPUT.with(|buf| std::mem::take(&mut *buf.borrow_mut()))
}
