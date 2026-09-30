use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use tint_ast::Span;
use tint_evaluator::errors::EvalError;
use tint_evaluator::value::Value;
use tint_runtime::vm::TintVM;

use crate::module_loader;
use crate::support::{has_fn, report_semantic_error, semantic_check};

pub(crate) fn command(path: Option<&String>, function: Option<&str>) {
    run_file(run_path(path), function.unwrap_or("main"));
}

fn run_path(path: Option<&String>) -> &str {
    path.map(String::as_str).unwrap_or("main.tn")
}

fn run_file(path: &str, function: &str) {
    let loaded = module_loader::load(path);
    let program = &loaded.program;

    for error in &semantic_check(&loaded) {
        eprintln!("warning: semantic:");
        report_semantic_error(&loaded.entry_source, error);
    }

    // `TINT_ENGINE=tree` forces the tree-walker; `native` makes a fallback an error.
    let engine = std::env::var("TINT_ENGINE").unwrap_or_default();
    if engine != "tree" && has_fn(&program, function) {
        match try_native(&loaded.program, function) {
            Ok(()) => return,
            Err(why) if engine == "native" => {
                eprintln!("native engine unavailable: {why}");
                std::process::exit(2);
            }
            Err(_) => {}
        }
    }

    let mut vm = TintVM::new();
    register_natives(&mut vm);
    vm.run_program(&program);

    if !has_fn(&program, function) && !vm.native_fns.borrow().contains_key(function) {
        println!(
            "compiled OK ({} item(s)), no `fn {}` to run",
            program.items.len(),
            function
        );
        return;
    }

    match vm.call_fn(function, &[], Span::dummy()) {
        Ok(_value) => {}
        Err(error) => {
            eprintln!("runtime error calling `{}`: {:?}", function, error);
            std::process::exit(1);
        }
    }
}

/// Compile the whole program to machine code and run `function`. Any reason the
/// program is outside the native subset (UI, `read_line`, unsupported types, ...)
/// comes back as `Err` before anything has run, so the caller can fall back.
#[cfg(not(feature = "native"))]
fn try_native(_program: &tint_ast::Program, _function: &str) -> Result<(), String> {
    Err("built without the `native` feature".into())
}

#[cfg(feature = "native")]
fn try_native(program: &tint_ast::Program, function: &str) -> Result<(), String> {
    use tint_semantics::prelude::CheckerContext;
    use tint_semantics::SemanticChecker;

    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(program);
    if !errors.is_empty() {
        return Err("type errors".into());
    }
    let lowered = tint_ir::typed::lower_program(program, &model);
    if !lowered.errors.is_empty() {
        return Err(format!("lowering: {:?}", lowered.errors));
    }
    let jit = tint_codegen::compile_for(lowered.module, &[function]).map_err(|e| e.to_string())?;
    jit.run_fn(function).map(|_| ())
}

fn register_natives(vm: &mut TintVM) {
    vm.register_native("print", |args| {
        let value = args
            .first()
            .ok_or_else(|| native_error("print expects one argument"))?;
        print!("{}", value);
        io::stdout()
            .flush()
            .map_err(|error| native_error(&format!("print failed: {error}")))?;
        Ok(Value::Unit)
    });

    vm.register_native("println", |args| {
        let value = args
            .first()
            .ok_or_else(|| native_error("println expects one argument"))?;
        println!("{}", value);
        Ok(Value::Unit)
    });

    vm.register_native("read_line", |_args| {
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .map_err(|error| native_error(&format!("read_line failed: {error}")))?;
        Ok(Value::String(
            line.trim_end_matches(['\r', '\n']).to_string(),
        ))
    });

    vm.register_native("parse_number", |args| {
        let value = match args.first() {
            Some(Value::String(value)) => value,
            _ => return Err(native_error("parse_number expects a string")),
        };
        match value.trim().parse::<f64>() {
            Ok(number) => Ok(Value::EnumInstance {
                enum_name: "Result".into(),
                variant: "Ok".into(),
                args: vec![Value::Number(number)],
            }),
            Err(error) => Ok(Value::EnumInstance {
                enum_name: "Result".into(),
                variant: "Err".into(),
                args: vec![Value::String(format!("invalid number `{value}`: {error}"))],
            }),
        }
    });

    vm.register_native("read_key", |_args| {
        terminal::enable_raw_mode().map_err(|error| {
            native_error(&format!("read_key failed to enable raw mode: {error}"))
        })?;

        let event =
            event::read().map_err(|error| native_error(&format!("read_key failed: {error}")));
        let restore = terminal::disable_raw_mode().map_err(|error| {
            native_error(&format!("read_key failed to restore terminal: {error}"))
        });
        restore?;

        let key = match event? {
            Event::Key(key) => key,
            _ => return Err(native_error("read_key received a non-key event")),
        };
        Ok(Value::String(format_key(key)))
    });

    vm.register_native("now_ms", |_args| {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(Value::Number(millis))
    });
}

fn native_error(message: &str) -> EvalError {
    EvalError::CallError {
        msg: message.to_string(),
        span: Span::dummy(),
    }
}

fn format_key(key: KeyEvent) -> String {
    let modifier = if key.modifiers.contains(KeyModifiers::CONTROL) {
        "Ctrl+"
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        "Alt+"
    } else {
        ""
    };

    let code = match key.code {
        KeyCode::Char('\u{1b}') => return "Escape".to_string(),
        KeyCode::Char('\u{3}') => return "Ctrl+c".to_string(),
        KeyCode::Char(character) => return format!("{modifier}{character}"),
        KeyCode::Enter => "Enter",
        KeyCode::Esc => "Escape",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::Up => "ArrowUp",
        KeyCode::Down => "ArrowDown",
        KeyCode::Left => "ArrowLeft",
        KeyCode::Right => "ArrowRight",
        KeyCode::Delete => "Delete",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::F(number) => return format!("{modifier}F{number}"),
        _ => "Unknown",
    };
    format!("{modifier}{code}")
}

#[cfg(test)]
mod tests {
    use super::{format_key, run_path};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn run_defaults_to_main_tn() {
        assert_eq!(run_path(None), "main.tn");
    }

    #[test]
    fn run_keeps_an_explicit_path() {
        let path = String::from("examples/temperature.tn");
        assert_eq!(run_path(Some(&path)), path);
    }

    #[test]
    fn formats_unicode_and_control_keys() {
        assert_eq!(
            format_key(KeyEvent::new(KeyCode::Char('é'), KeyModifiers::NONE)),
            "é"
        );
        assert_eq!(
            format_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            "Enter"
        );
        assert_eq!(
            format_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            "Ctrl+c"
        );
        assert_eq!(
            format_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            "ArrowUp"
        );
    }
}
