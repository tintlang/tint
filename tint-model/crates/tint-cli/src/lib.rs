//! The `tint` command line as a library, so a generated runner can add Rust
//! host functions (`app { rs::"./native.rs" }`) and reuse the same commands.

mod build;
mod prerender;
mod dev;
mod module_loader;
mod native_runner;
mod repl;
mod run;
mod support;
mod wasm_app;
mod wasm_build;

use std::cell::RefCell;

pub use tint_runtime::ui_session::NativeFn;

thread_local! {
    static USER_NATIVES: RefCell<Vec<(String, NativeFn)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn user_natives() -> Vec<(String, NativeFn)> {
    USER_NATIVES.with(|natives| natives.borrow().clone())
}

/// Runs the CLI (`args[0]` is the program name) with extra host functions
/// callable from `.tn` code.
pub fn main_with(args: Vec<String>, natives: Vec<(String, NativeFn)>) {
    USER_NATIVES.with(|slot| *slot.borrow_mut() = natives);

    match args.get(1).map(String::as_str) {
        Some("run") => run::command(args.get(2), args.get(3).map(String::as_str)),
        Some("check") => support::check_command(&args[2..]),
        Some("build") => build::command(&args),
        Some("dev") => dev::command(&args),
        Some("repl") | None => repl::command(),
        Some(other) => {
            eprintln!(
                "unknown command '{}'. usage: tint <run|check [--strict]|build|dev|repl> [args]",
                other
            );
            std::process::exit(1);
        }
    }
}
