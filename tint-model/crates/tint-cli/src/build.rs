use std::fs;
use std::path::Path;

use crate::support::{parse_source, read_file, report_parse_error, require_arg};

const TINT_WASM_JS: &str = include_str!("../embedded/tint_wasm.js");
const TINT_WASM_BG: &[u8] = include_bytes!("../embedded/tint_wasm_bg.wasm");

pub(crate) fn command(args: &[String]) {
    let path = require_arg(args.get(2), "tint build <file.tn> [ui_fn] -o <output.html>");
    let (entry, output) = parse_options(args);
    build_file(path, &entry, output.as_deref());
}

fn parse_options(args: &[String]) -> (String, Option<String>) {
    let mut entry = "App".to_string();
    let mut output = None;
    let mut index = 3;

    while index < args.len() {
        if args[index] == "-o" {
            output = args.get(index + 1).cloned();
            index += 2;
        } else {
            entry = args[index].clone();
            index += 1;
        }
    }

    (entry, output)
}

fn build_file(path: &str, entry: &str, output: Option<&str>) {
    let source = read_file(path);
    if let Err(error) = parse_source(&source) {
        report_parse_error(&source, &error);
        std::process::exit(1);
    }

    let output = output.map(str::to_owned).unwrap_or_else(|| {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        format!("{}.html", stem)
    });

    if let Err(error) = fs::write(&output, standalone_html(&source, entry)) {
        eprintln!("error writing '{}': {}", output, error);
        std::process::exit(1);
    }

    println!("built: {} (entry: `ui fn {}`)", output, entry);
}

fn standalone_html(source: &str, entry: &str) -> String {
    let source = source
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");
    let wasm = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, TINT_WASM_BG);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tint UI</title>
    <style>
        * {{ box-sizing: border-box; }}
        body {{ margin: 0; padding: 16px; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }}
        .error {{ background: #fee; border: 1px solid #fcc; border-radius: 4px; padding: 16px; color: #c00; font-family: monospace; white-space: pre-wrap; }}
        .loading {{ padding: 40px 20px; color: #666; }}
    </style>
</head>
<body>
    <div id="app" class="loading">Loading Tint UI...</div>
    <script type="module">
{TINT_WASM_JS}
        const SOURCE = `{source}`;
        const ENTRY = "{entry}";
        const WASM_BASE64 = "{wasm}";
        function base64ToBytes(b64) {{
            const bin = atob(b64);
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return bytes;
        }}
        const app = document.getElementById('app');
        try {{
            initSync({{ module: base64ToBytes(WASM_BASE64) }});
            app.className = '';
            app.textContent = '';
            const session = new DomSession(SOURCE, ENTRY, 'app');
            const error = session.rerender();
            if (error) {{
                app.innerHTML = '<div class="error"></div>';
                app.firstChild.textContent = 'Error rendering `ui fn {entry}`: ' + error;
            }}
        }} catch (error) {{
            app.innerHTML = '<div class="error"></div>';
            app.firstChild.textContent = 'Error loading Tint runtime: ' + error.message;
        }}
    </script>
</body>
</html>
"#
    )
}
