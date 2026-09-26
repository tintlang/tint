mod build;
mod repl;
mod run;
mod support;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("run") => run::command(args.get(2), args.get(3).map(String::as_str)),
        Some("check") => support::check_command(args.get(2)),
        Some("build") => build::command(&args),
        Some("repl") | None => repl::command(),
        Some(other) => {
            eprintln!(
                "unknown command '{}'. usage: tint <run|check|build|repl> [args]",
                other
            );
            std::process::exit(1);
        }
    }
}
