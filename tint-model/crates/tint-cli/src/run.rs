use std::time::{SystemTime, UNIX_EPOCH};

use tint_ast::Span;
use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_runtime::vm::TintVM;

use crate::module_loader;
use crate::support::{has_fn, report_semantic_error, require_arg, semantic_check};

pub(crate) fn command(path: Option<&String>, function: Option<&str>) {
    let path = require_arg(path, "tint run <file.tn> [fn_name]");
    run_file(path, function.unwrap_or("main"));
}

fn run_file(path: &str, function: &str) {
    let loaded = module_loader::load(path);
    let program = loaded.program;

    for error in &semantic_check(&program) {
        eprintln!("warning: semantic:");
        report_semantic_error(&loaded.entry_source, error);
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
        Ok(value) => println!("{} => {}", function, value),
        Err(error) => {
            eprintln!("runtime error calling `{}`: {:?}", function, error);
            std::process::exit(1);
        }
    }
}

fn register_natives(vm: &mut TintVM) {
    vm.register_native("now_ms", |_args| {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(Value::Number(millis))
    });
}
