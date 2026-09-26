// tint-cli/src/main.rs
//
// First concrete piece of the "sandbox" plan: until now the only way to
// run any Tint source was Rust's own test harness
// (tint-runtime/tests/main.rs) -- there was no binary at all in the
// workspace. This gives Mark a `tint` executable with three commands:
//
//   tint check <file.tn>            -- lex + parse only, report the error
//                                       (with span/snippet) or how many
//                                       top-level items were found.
//   tint run <file.tn> [fn_name]    -- parse, compile to IR, load into a
//                                       TintVM, and call `fn_name` (default
//                                       "main") with no arguments, printing
//                                       its result via Value's Display.
//   tint build <file.tn> -o out.html -- compile UI code to standalone HTML
//   tint repl                       -- a minimal REPL: each entry is
//                                       compiled and run in its OWN fresh
//                                       TintVM (no state persists between
//                                       entries yet -- see the comment in
//                                       cmd_repl for why, and what a
//                                       persistent version would need).
//
// This is deliberately thin: it exercises the EXISTING lexer -> parser ->
// SsaCompiler -> IrVM / EvalHost pipeline exactly as the test suite does,
// just from a real command line instead of a #[test] function.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use tint_ast::{Item, Program, Span};
use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("run") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: tint run <file.tn> [fn_name]");
                std::process::exit(1);
            };
            let fn_name = args.get(3).map(String::as_str).unwrap_or("main");
            cmd_run(path, fn_name);
        }

        Some("check") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: tint check <file.tn>");
                std::process::exit(1);
            };
            cmd_check(path);
        }

        Some("build") => {
            let Some(path) = args.get(2) else {
                eprintln!("usage: tint build <file.tn> -o <output.html>");
                std::process::exit(1);
            };
            let output = if args.get(3).map(String::as_str) == Some("-o") {
                args.get(4).cloned()
            } else {
                None
            };
            cmd_build(path, output.as_deref());
        }

        Some("repl") | None => cmd_repl(),

        Some(other) => {
            eprintln!(
                "unknown command '{}'. usage: tint <run|check|build|repl> [args]",
                other
            );
            std::process::exit(1);
        }
    }
}

fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading '{}': {}", path, e);
            std::process::exit(1);
        }
    }
}

fn parse_source(code: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

// Mirrors the error-reporting style tint-runtime/tests/main.rs's
// assert_parses() already uses: print the error plus the exact source
// snippet the span points at, not just a raw Debug dump.
fn report_parse_error(code: &str, e: &ParserError) {
    eprintln!("parse error: {:?}", e);

    let span = match e {
        ParserError::Message { span, .. } => Some(*span),
        ParserError::Unexpected { span, .. } => Some(*span),
    };

    if let Some(span) = span {
        let s = span.start.offset.min(code.len());
        let en = span.end.offset.min(code.len());
        eprintln!(
            "  at {}:{} -> `{}`",
            span.start.line,
            span.start.column,
            &code[s..en]
        );
    }
}

fn cmd_check(path: &str) {
    let code = read_file(path);

    match parse_source(&code) {
        Ok(program) => println!("OK: {} top-level item(s) parsed", program.items.len()),
        Err(e) => {
            report_parse_error(&code, &e);
            std::process::exit(1);
        }
    }
}

fn cmd_run(path: &str, fn_name: &str) {
    let code = read_file(path);

    let program = match parse_source(&code) {
        Ok(p) => p,
        Err(e) => {
            report_parse_error(&code, &e);
            std::process::exit(1);
        }
    };

    let mut vm = TintVM::new();
    register_natives(&mut vm);
    vm.run_program(&program);

    if !has_fn(&program, fn_name) && !vm.native_fns.borrow().contains_key(fn_name) {
        println!(
            "compiled OK ({} item(s)), no `fn {}` to run",
            program.items.len(),
            fn_name
        );
        return;
    }

    // No CLI-supplied arguments yet -- `fn_name` is called with an empty
    // argument list, so this covers a `main()`-style entry point (or any
    // other no-arg function picked via the optional third CLI arg).
    // Passing arguments from the command line is a reasonable follow-up,
    // left out for now rather than guessed at.
    match vm.call_fn(fn_name, &[], Span::dummy()) {
        Ok(v) => println!("{} => {}", fn_name, v),
        Err(e) => {
            eprintln!("runtime error calling `{}`: {:?}", fn_name, e);
            std::process::exit(1);
        }
    }
}

fn has_fn(program: &Program, name: &str) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(f) if f.name == name))
}

/// Demo native Rust functions, registered on every `tint run` -- direct
/// Rust interop (`TintVM::register_native`): call real Rust from `.tn`
/// source under a plain name, no Rust syntax inside the language at all.
/// `now_ms` in particular does something Tint itself has no way to
/// express (reading the system clock), so `tint run <file> now_ms` (with
/// no matching `fn now_ms` in the file) is a concrete, runnable proof this
/// reaches all the way from the CLI into real Rust and back.
fn register_natives(vm: &mut TintVM) {
    use std::time::{SystemTime, UNIX_EPOCH};

    vm.register_native("now_ms", |_args| {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        Ok(Value::Number(ms))
    });
}

fn cmd_build(path: &str, output: Option<&str>) {
    let code = read_file(path);

    // Verify the file parses
    if let Err(e) = parse_source(&code) {
        report_parse_error(&code, &e);
        std::process::exit(1);
    }

    // Determine output path
    let out_path = if let Some(o) = output {
        o.to_string()
    } else {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        format!("{}.html", stem)
    };

    // Generate standalone HTML
    let html = generate_standalone_html(&code);

    // Write output
    if let Err(e) = fs::write(&out_path, html) {
        eprintln!("error writing '{}': {}", out_path, e);
        std::process::exit(1);
    }

    println!("built: {}", out_path);
}

fn generate_standalone_html(source_code: &str) -> String {
    // Escape the source code for embedding in a JavaScript string
    let escaped_source = source_code
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");

    // HTML template with embedded WASM bootstrap
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tint UI</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #f5f7fa;
            padding: 16px;
            color: #20232d;
        }}
        
        #app {{
            max-width: 1200px;
            margin: 0 auto;
        }}
        
        .tint-ui-node {{
            display: contents;
        }}
        
        /* Default tag styles */
        [data-tag="Row"] {{
            display: flex;
            flex-direction: row;
            align-items: center;
            gap: 6px;
        }}
        
        [data-tag="Column"] {{
            display: flex;
            flex-direction: column;
            gap: 6px;
        }}
        
        [data-tag="Button"] {{
            display: inline-block;
            padding: 6px 14px;
            background: #6c5ce7;
            color: white;
            border-radius: 4px;
            border: none;
            cursor: pointer;
            font-weight: 600;
            font-size: 14px;
        }}
        
        [data-tag="Button"]:hover {{
            opacity: 0.9;
            transform: translateY(-1px);
        }}
        
        [data-tag="Card"] {{
            background: #ffffff;
            border-radius: 8px;
            padding: 12px;
            box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
        }}
        
        [data-tag="Text"] {{
            display: contents;
        }}
        
        [data-tag="Block"] {{
            animation: fadeIn 0.3s ease-in-out;
        }}
        
        @keyframes fadeIn {{
            from {{
                opacity: 0;
                transform: translateY(8px);
            }}
            to {{
                opacity: 1;
                transform: translateY(0);
            }}
        }}
        
        .error {{
            background: #fee;
            border: 1px solid #fcc;
            border-radius: 4px;
            padding: 16px;
            color: #c00;
            font-family: monospace;
        }}
        
        .loading {{
            text-align: center;
            padding: 40px 20px;
            color: #666;
        }}
    </style>
</head>
<body>
    <div id="app" class="loading">Loading Tint UI...</div>
    
    <script>
        // Source code to compile and render
        const SOURCE = `{escaped_source}`;
        
        // WASM module initialization - will be injected by build process
        // For now, this is a stub that shows how it would work
        async function initWasm() {{
            try {{
                // The actual WASM module would be embedded here
                // This requires wasm-pack build output
                console.log('Note: Standalone HTML generation requires WASM binary.');
                console.log('Use the web sandbox for interactive testing.');
                document.getElementById('app').innerHTML = 
                    '<div class="error">WASM binary not yet embedded in standalone build. Please use the web IDE.</div>';
            }} catch (err) {{
                document.getElementById('app').innerHTML = 
                    '<div class="error">Error loading WASM: ' + err.message + '</div>';
            }}
        }}
        
        // Recursively render UI tree to DOM
        function renderNode(node) {{
            if (node.tag === "Text" && node.text != null) {{
                return document.createTextNode(node.text);
            }}
            
            const elem = document.createElement("div");
            elem.className = "tint-ui-node";
            elem.dataset.tag = node.tag;
            
            // Apply inline styles
            if (node.style && node.style.length > 0) {{
                const styles = Object.fromEntries(node.style);
                Object.assign(elem.style, styles);
            }}
            
            // Add children
            if (node.children && node.children.length > 0) {{
                for (const child of node.children) {{
                    elem.appendChild(renderNode(child));
                }}
            }}
            
            return elem;
        }}
        
        // Initialize on load
        window.addEventListener('load', initWasm);
    </script>
</body>
</html>
"#
    )
}

fn cmd_repl() {
    println!("tint repl -- type Tint code, blank line to run, `:q` to quit");
    // Each entry compiles and runs in a brand-new TintVM: `struct`/`enum`
    // declarations and top-level `fn`s from a PREVIOUS entry aren't
    // visible to the next one. A persistent REPL would need to keep
    // accumulating source across entries (and re-run the growing program
    // each time, since there's no incremental-compile API yet) -- doable,
    // but a bigger step than this first pass; called out here rather than
    // silently limited.
    println!("(state does not persist between entries yet)");

    loop {
        print!("tint> ");
        if io::stdout().flush().is_err() {
            return;
        }

        let mut buf = String::new();
        loop {
            let mut line = String::new();
            if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
                println!();
                return; // EOF (Ctrl-D)
            }
            if line.trim() == ":q" {
                return;
            }
            if line.trim().is_empty() {
                break;
            }
            buf.push_str(&line);
        }

        if buf.trim().is_empty() {
            continue;
        }

        // A top-level item (fn/struct/enum/...) is run as-is, looking for
        // `main`. Anything else is treated as a statement/expression body
        // and wrapped in a throwaway function, so a bare `2 + 2` or
        // `let x = 5; x * x` works without the user having to write `fn`
        // boilerplate every time.
        let looks_like_item = ["fn ", "struct ", "enum ", "ui fn", "space ", "impl "]
            .iter()
            .any(|kw| buf.trim_start().starts_with(kw));

        let (source, entry) = if looks_like_item {
            (buf.clone(), "main")
        } else {
            (format!("fn __repl__() {{\n{}\n}}", buf), "__repl__")
        };

        let program = match parse_source(&source) {
            Ok(p) => p,
            Err(e) => {
                report_parse_error(&source, &e);
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
            Ok(v) => println!("=> {}", v),
            Err(e) => eprintln!("error: {:?}", e),
        }
    }
}
