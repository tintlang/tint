use std::fs;
use std::path::Path;

use crate::module_loader;
use crate::support::{require_arg, semantic_check};

const TINT_WASM_JS: &str = include_str!("../embedded/tint_wasm.js");
const TINT_WASM_BG: &[u8] = include_bytes!("../embedded/tint_wasm_bg.wasm.gz");

pub(crate) fn command(args: &[String]) {
    let path = require_arg(args.get(2), "tint build <file.tn> [ui_fn] -o <output.html>");
    let (entry, output) = parse_options(args);
    build_file(path, entry.as_deref(), output.as_deref());
}

fn parse_options(args: &[String]) -> (Option<String>, Option<String>) {
    let mut entry = None;
    let mut output = None;
    let mut index = 3;

    while index < args.len() {
        if args[index] == "-o" {
            output = args.get(index + 1).cloned();
            index += 2;
        } else {
            entry = Some(args[index].clone());
            index += 1;
        }
    }

    (entry, output)
}

/// The `ui fn` to mount when the user didn't name one: `App` if the entry
/// file declares it, otherwise the first `ui fn` in the file.
pub(crate) fn infer_entry(entry_source: &str) -> String {
    let names: Vec<&str> = entry_source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("ui fn "))
        .filter_map(|rest| {
            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
        })
        .filter(|name| !name.is_empty())
        .collect();
    if names.contains(&"App") {
        "App".to_string()
    } else {
        names.first().map_or("App", |name| name).to_string()
    }
}

fn build_file(path: &str, entry: Option<&str>, output: Option<&str>) {
    // Loading (not just parsing) validates `mod`/`use` the same way `run`/
    // `check` do, and gives us every reachable file's raw source to embed
    // -- see `standalone_html`'s doc comment on why plain concatenation of
    // those texts is enough, with no unparser needed.
    let loaded = module_loader::load(path);
    for error in &semantic_check(&loaded) {
        eprintln!("warning: semantic:");
        crate::support::report_semantic_error(&loaded.entry_source, error);
    }
    let combined_source = loaded.all_sources.join("\n\n");
    let entry = entry.map_or_else(|| infer_entry(&loaded.entry_source), str::to_owned);
    let entry = entry.as_str();

    let output = output.map(str::to_owned).unwrap_or_else(|| {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output");
        format!("{}.html", stem)
    });

    if let Some(dir) = Path::new(&output).parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(error) = fs::create_dir_all(dir) {
            eprintln!("error creating '{}': {}", dir.display(), error);
            std::process::exit(1);
        }
    }

    if let Err(error) = fs::write(
        &output,
        standalone_html(&combined_source, entry, &loaded.program.app_meta()),
    ) {
        eprintln!("error writing '{}': {}", output, error);
        std::process::exit(1);
    }

    println!("built: {} (entry: `ui fn {}`)", output, entry);
}

/// `source` here is every reachable file's raw text concatenated together
/// (see `Loaded::all_sources`), not just the entry file's. This works with
/// zero extra handling on the browser side: `DomSession`'s wasm-side parser
/// has no filesystem and no module loader at all, so `mod name;`/`use
/// a::b;` lines just parse into `Item::Mod`/`Item::Use` nodes that nothing
/// downstream (`register_functions`, the IR compiler, the checker) ever
/// acts on -- harmless no-ops -- while the actual functions/structs/enums
/// from every module are already present verbatim in the blob, directly
/// callable by their bare name, exactly as the CLI-side loader's own
/// flattening already relies on the VM being one flat namespace.
pub(crate) fn standalone_html(source: &str, entry: &str, meta: &tint_ast::AppMeta) -> String {
    let title = meta.title.as_deref().unwrap_or("Tint UI");
    let title = title.replace('&', "&amp;").replace('<', "&lt;");
    let lang = meta.lang.as_deref().unwrap_or("en");
    let lang = lang.replace('"', "");
    let page_css: String = tint_runtime::ui::style::resolve_page_style(&meta.page)
        .iter()
        .map(|(property, value)| format!(" {}: {};", property, value))
        .collect();
    // An app that declares `page::{ }` owns the whole page box; without it the
    // shell keeps the old comfortable default.
    let page_base = if page_css.is_empty() {
        " margin: 0; padding: 16px;"
    } else {
        " margin: 0;"
    };
    let at_rules = tint_runtime::ui::style::app_at_rules(meta);
    let source = source
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
        // A `</script>` inside the source (say, an embedded HTML sample) must
        // not end the page's own script element.
        .replace("</", "<\\/");
    let wasm = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, TINT_WASM_BG);


    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        * {{ box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }}
        body {{{page_base}{page_css} }}
        {at_rules}
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
            if (Uint8Array.fromBase64) return Uint8Array.fromBase64(b64);
            const bin = atob(b64);
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return bytes;
        }}
        const app = document.getElementById('app');
        try {{
            // The runtime is embedded gzipped; inflate it before instantiating.
            const wasmBytes = new Uint8Array(await new Response(
                new Blob([base64ToBytes(WASM_BASE64)]).stream().pipeThrough(new DecompressionStream('gzip'))
            ).arrayBuffer());
            initSync({{ module: wasmBytes }});
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

#[cfg(test)]
mod tests {
    use super::standalone_html;

    #[test]
    fn embedded_source_cannot_close_the_script_element() {
        let html = standalone_html(
            "fn s() = \"<script src='x'></script>\"\nui fn App() {}",
            "App",
            &tint_ast::AppMeta::default(),
        );
        assert!(!html.contains("'x'></script>"));
        assert!(html.contains("<\\/script>"));
    }
}
