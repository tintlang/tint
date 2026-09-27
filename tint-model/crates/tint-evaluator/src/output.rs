// Captured output for hosts without useful stdout, such as the browser.
// Thread-local storage keeps concurrent evaluator runs isolated.

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
