use std::io::{self, Write};

use tint_ast::Span;
use tint_evaluator::EvalHost;
use tint_runtime::vm::TintVM;

use crate::support::{has_fn, parse_source, report_parse_error};

pub(crate) fn command() {
    println!("tint repl -- type Tint code, blank line to run, `:q` to quit");
    println!("(state does not persist between entries yet)");

    loop {
        print!("tint> ");
        if io::stdout().flush().is_err() {
            return;
        }

        let Some(source) = read_entry() else {
            return;
        };
        if source.trim().is_empty() {
            continue;
        }

        let looks_like_item = ["fn ", "struct ", "enum ", "ui fn", "space ", "impl "]
            .iter()
            .any(|keyword| source.trim_start().starts_with(keyword));
        let (source, entry) = if looks_like_item {
            (source, "main")
        } else {
            (format!("fn __repl__() {{\n{}\n}}", source), "__repl__")
        };

        let program = match parse_source(&source) {
            Ok(program) => program,
            Err(error) => {
                report_parse_error(&source, &error);
                continue;
            }
        };

        let mut vm = TintVM::new();
        vm.run_program(&program);
        if !has_fn(&program, entry) {
            println!("defined.");
            continue;
        }

        match vm.call_fn(entry, &[], Span::dummy()) {
            Ok(value) => println!("=> {}", value),
            Err(error) => eprintln!("error: {:?}", error),
        }
    }
}

fn read_entry() -> Option<String> {
    let mut source = String::new();
    loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            return None;
        }
        if line.trim() == ":q" {
            return None;
        }
        if line.trim().is_empty() {
            return Some(source);
        }
        source.push_str(&line);
    }
}
