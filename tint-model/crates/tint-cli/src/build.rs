use std::fs;
use std::path::Path;

use crate::module_loader;
use crate::support::{require_arg, semantic_check};

const TINT_WASM_JS: &str = include_str!("../embedded/tint_wasm.js");
const TINT_WASM_BG: &[u8] = include_bytes!("../embedded/tint_wasm_bg.wasm.gz");
// Runtimes of compiled apps (scripts/build-cli-runtimes.sh).
const RT_DOM_JS: &str = include_str!("../embedded/rt_dom.js");
const RT_DOM_WASM: &[u8] = include_bytes!("../embedded/rt_dom_bg.wasm.gz");
const RT_DOM_FULL_JS: &str = include_str!("../embedded/rt_dom_full.js");
const RT_DOM_FULL_WASM: &[u8] = include_bytes!("../embedded/rt_dom_full_bg.wasm.gz");

const USAGE: &str = "tint build <file.tn> [ui_fn] [-o <output.html>] [--engine wasm|interpreter]";

/// How the page runs the program.
#[derive(Clone, Copy, PartialEq)]
enum Engine {
    /// The program compiled to WebAssembly (default).
    Wasm,
    /// The source embedded and run by the interpreter in the page.
    Interpreter,
}

pub(crate) fn command(args: &[String]) {
    let path = require_arg(args.get(2), USAGE);
    let (entry, output, engine) = parse_options(args);
    build_file(path, entry.as_deref(), output.as_deref(), engine);
}

fn parse_options(args: &[String]) -> (Option<String>, Option<String>, Engine) {
    let mut entry = None;
    let mut output = None;
    let mut engine = Engine::Wasm;
    let mut index = 3;

    while index < args.len() {
        if args[index] == "-o" {
            output = args.get(index + 1).cloned();
            index += 2;
        } else if args[index] == "--engine" {
            engine = match args.get(index + 1).map(String::as_str) {
                Some("wasm") => Engine::Wasm,
                Some("interpreter") => Engine::Interpreter,
                _ => {
                    eprintln!("usage: {USAGE}");
                    std::process::exit(1);
                }
            };
            index += 2;
        } else {
            entry = Some(args[index].clone());
            index += 1;
        }
    }

    (entry, output, engine)
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

fn build_file(path: &str, entry: Option<&str>, output: Option<&str>, engine: Engine) {
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

    let meta = loaded.program.app_meta();
    let mut inline = read_inline_assets(path, &meta).unwrap_or_else(|error| {
        eprintln!("{}", error);
        std::process::exit(1);
    });
    inline.loading = meta
        .loading
        .as_deref()
        .and_then(|entry| crate::prerender::render(&combined_source, entry));
    let html = match engine {
        Engine::Interpreter => standalone_html(&combined_source, entry, &meta, &inline),
        Engine::Wasm => match crate::wasm_app::compile(&loaded) {
            Ok(app) => {
                // A page may start from a `ui fn` the entry file does not declare (routes).
                let start = if app.ui_fns.iter().any(|name| name == entry) { entry } else { app.ui_fns[0].as_str() };
                println!("compiled: {} KB of WebAssembly", app.wasm.len() / 1024);
                inline.prerender = crate::prerender::render(&combined_source, start);
                match &inline.prerender {
                    Some(html) => println!("prerendered: {} KB of HTML", html.len() / 1024),
                    None => println!("prerender skipped: `{start}` could not run at build time"),
                }
                compiled_html(&app, start, &meta, &inline)
            }
            Err(message) => {
                eprintln!("error: {message}");
                std::process::exit(1);
            }
        },
    };
    if let Err(error) = fs::write(&output, html) {
        eprintln!("error writing '{}': {}", output, error);
        std::process::exit(1);
    }

    println!("built: {}", output);
}

/// File contents named by `app { js::"..." css::"..." }`, embedded into the page.
#[derive(Default)]
pub(crate) struct InlineAssets {
    /// `rs::` files compiled to wasm.
    pub wasm: Option<std::sync::Arc<crate::wasm_build::WasmAssets>>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    /// The first screen as static HTML (`prerender.rs`), shown until the runtime has loaded.
    pub prerender: Option<String>,
    /// Optional user-defined loading screen from `app { loading::UiFn }`.
    pub loading: Option<String>,
}

/// Reads the declared files, resolving paths against the entry file's folder.
/// A JS module is embedded as one blob, so it cannot import other files.
pub(crate) fn read_inline_assets(
    entry_path: &str,
    meta: &tint_ast::AppMeta,
) -> Result<InlineAssets, String> {
    let wasm = if meta.rs.is_empty() {
        None
    } else {
        Some(crate::wasm_build::build(entry_path, &meta.rs)?)
    };
    let base = Path::new(entry_path).parent().unwrap_or(Path::new(""));
    let read = |name: &String| {
        fs::read_to_string(base.join(name))
            .map_err(|error| format!("error reading '{}': {}", base.join(name).display(), error))
    };
    Ok(InlineAssets {
        wasm,
        js: meta.js.iter().map(read).collect::<Result<_, _>>()?,
        css: meta.css.iter().map(read).collect::<Result<_, _>>()?,
        prerender: None,
        loading: None,
    })
}

fn js_string(text: &str) -> String {
    // JSON string literal, safe inside a <script> element.
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '<' => out.push_str("\\u003c"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
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
pub(crate) fn standalone_html(
    source: &str,
    entry: &str,
    meta: &tint_ast::AppMeta,
    inline: &InlineAssets,
) -> String {
    let source = source
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
        // A `</script>` inside the source (say, an embedded HTML sample) must
        // not end the page's own script element.
        .replace("</", "<\\/");
    let wasm = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, TINT_WASM_BG);
    let script = format!(
        r#"{TINT_WASM_JS}
        const SOURCE = `{source}`;
        const ENTRY = "{entry}";
        {PAGE_HELPERS}
        const WASM_BASE64 = "{wasm}";
        const app = document.getElementById('app');
        try {{
            initSync({{ module: await gunzip(WASM_BASE64) }});
            app.className = '';
            app.textContent = '';
            const natives = await loadNatives();
            const session = new DomSession(SOURCE, ENTRY, 'app', natives);
            const error = session.rerender();
            if (error) {{
                app.innerHTML = '<div class="error"></div>';
                app.firstChild.textContent = 'Error rendering `ui fn {entry}`: ' + error;
            }}
        }} catch (error) {{
            app.innerHTML = '<div class="error"></div>';
            app.firstChild.textContent = 'Error loading Tint runtime: ' + error.message;
        }}"#
    );
    page(meta, inline, &script)
}

/// The page of a program compiled to WebAssembly: the runtime (`tint-wasmrt` with the DOM layer),
/// the app module, and the glue that connects them.
pub(crate) fn compiled_html(
    app: &crate::wasm_app::CompiledApp,
    entry: &str,
    meta: &tint_ast::AppMeta,
    inline: &InlineAssets,
) -> String {
    let (glue, runtime) = if app.needs_interpreter {
        (RT_DOM_FULL_JS, RT_DOM_FULL_WASM)
    } else {
        (RT_DOM_JS, RT_DOM_WASM)
    };
    let b64 = |bytes: &[u8]| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    let runtime = b64(runtime);
    let app_wasm = b64(&gzip(&app.wasm));
    let script = format!(
        r#"{glue}
        const ENTRY = "{entry}";
        {PAGE_HELPERS}
        const RUNTIME_BASE64 = "{runtime}";
        const APP_BASE64 = "{app_wasm}";
        const app = document.getElementById('app');
        try {{
            // The runtime exports its memory and functions; the app module imports them.
            const rt = initSync({{ module: await gunzip(RUNTIME_BASE64) }});
            compiled_prepare(await loadNatives());
            const {{ instance }} = await WebAssembly.instantiate(await gunzip(APP_BASE64), {{ rt }});
            app.className = '';
            app.textContent = '';
            const session = DomSession.from_compiled(instance.exports, ENTRY, 'app');
            const error = session.rerender();
            if (error) {{
                app.innerHTML = '<div class="error"></div>';
                app.firstChild.textContent = 'Error rendering `ui fn {entry}`: ' + error;
            }}
        }} catch (error) {{
            app.innerHTML = '<div class="error"></div>';
            app.firstChild.textContent = 'Error loading Tint app: ' + error.message;
        }}"#
    );
    page(meta, inline, &script)
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes).expect("gzip to memory");
    encoder.finish().expect("gzip to memory")
}

/// Shared by both pages: decoding embedded bytes and loading the natives
/// (`app { js::".." rs::".." }`, available as `USER_JS` / `USER_WASM`).
const PAGE_HELPERS: &str = r#"
        function base64ToBytes(b64) {
            if (Uint8Array.fromBase64) return Uint8Array.fromBase64(b64);
            const bin = atob(b64);
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return bytes;
        }
        async function gunzip(b64) {
            return new Uint8Array(await new Response(
                new Blob([base64ToBytes(b64)]).stream().pipeThrough(new DecompressionStream('gzip'))
            ).arrayBuffer());
        }
        // Exported functions of `app { js::"..." }` modules, callable from .tn.
        async function loadNatives() {
            const natives = {};
            if (USER_WASM) {
                // `rs::` files, compiled to wasm: `__tint_<name>(args)` per export.
                const url = URL.createObjectURL(new Blob([USER_WASM.glue], { type: 'text/javascript' }));
                const mod = await import(url);
                URL.revokeObjectURL(url);
                mod.initSync({ module: await gunzip(USER_WASM.data) });
                for (const [name, value] of Object.entries(mod)) {
                    if (name.startsWith('__tint_') && typeof value === 'function') {
                        natives[name.slice(7)] = (...args) => value(args);
                    }
                }
            }
            for (const code of USER_JS) {
                const url = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
                const mod = await import(url);
                URL.revokeObjectURL(url);
                for (const [name, value] of Object.entries(mod)) {
                    if (typeof value === 'function') natives[name] = value;
                }
            }
            return natives;
        }"#;

/// The HTML around a page's script: title, page styles, the user's CSS and the
/// `USER_JS` / `USER_WASM` data `PAGE_HELPERS` reads.
fn page(meta: &tint_ast::AppMeta, inline: &InlineAssets, script: &str) -> String {
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
    let user_css: String = inline
        .css
        .iter()
        .map(|css| format!("    <style>\n{}\n    </style>\n", css.replace("</", "<\\/")))
        .collect();
    let user_js = inline.js.iter().map(|js| js_string(js)).collect::<Vec<_>>().join(", ");
    let user_wasm = match &inline.wasm {
        Some(assets) => format!(
            "{{ glue: {}, data: \"{}\" }}",
            js_string(&assets.glue),
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &assets.wasm_gz)
        ),
        None => "null".to_string(),
    };

    // A prerendered first screen belongs to `/`: on another path it is replaced by the user's
    // optional loading screen. Without one, the shell stays empty until the selected route mounts.
    let loading_html = inline.loading.as_deref().unwrap_or("");
    let app_div = match &inline.prerender {
        Some(html) => format!(
            "<div id=\"app\" class=\"app-shell\" data-tint-route=\"/\">{html}</div>\n    <script>{{const a=document.getElementById('app');if(location.pathname!=='/'){{a.innerHTML={};a.removeAttribute('data-tint-route')}}}}</script>",
            js_string(loading_html)
        ),
        None => format!(r#"<div id="app" class="app-shell">{loading_html}</div>"#),
    };
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        * {{ box-sizing: border-box; }}
        html, body {{ background: #0a0a0a; }}
        html[data-tint-theme="light"], html[data-tint-theme="light"] body {{ background: #ffffff; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }}
        body {{{page_base}{page_css} }}
        {at_rules}
        .error {{ background: #fee; border: 1px solid #fcc; border-radius: 4px; padding: 16px; color: #c00; font-family: monospace; white-space: pre-wrap; }}
        .app-shell {{ min-height: 100dvh; background: inherit; }}
    </style>
{user_css}</head>
<body>
    <script>
        try {{ document.documentElement.dataset.tintTheme = localStorage.getItem('theme') === 'light' ? 'light' : 'dark'; }}
        catch (_) {{ document.documentElement.dataset.tintTheme = 'dark'; }}
    </script>
    {app_div}
    <script type="module">
        const USER_JS = [{user_js}];
        const USER_WASM = {user_wasm};
{script}
    </script>
</body>
</html>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::{compiled_html, standalone_html, InlineAssets};

    fn load(name: &str, source: &str) -> crate::module_loader::Loaded {
        let path = std::env::temp_dir().join(format!("tint_build_test_{}_{name}.tn", std::process::id()));
        std::fs::write(&path, source).unwrap();
        let loaded = crate::module_loader::try_load(path.to_str().unwrap()).expect("loads");
        std::fs::remove_file(&path).ok();
        loaded
    }

    #[test]
    fn default_page_runs_a_compiled_module() {
        let loaded = load("ok", "fn inc() { n = n + 1 }\nui fn App() { state n = 0\n Button { click||inc \"{n}\" } }");
        let app = crate::wasm_app::compile(&loaded).expect("compiles");
        assert!(app.wasm.starts_with(b"\0asm"));
        assert!(!app.needs_interpreter);
        let html = compiled_html(&app, "App", &tint_ast::AppMeta::default(), &InlineAssets::default());
        assert!(html.contains("DomSession.from_compiled") && html.contains("const APP_BASE64"));
        assert!(!html.contains("new DomSession(SOURCE"));
    }

    #[test]
    fn preview_pages_carry_the_interpreter() {
        let loaded = load("preview", r#"ui fn App() {
    state s = "x"
    Column { Preview { entry||"App" "{s}" } }
}"#);
        let app = crate::wasm_app::compile(&loaded).expect("compiles");
        assert!(app.needs_interpreter);
    }

    #[test]
    fn unsupported_programs_are_build_errors() {
        let loaded = load("host", "ui fn App() { state s = fetch_it(1)\n Text { \"{s}\" } }");
        let message = crate::wasm_app::compile(&loaded).err().expect("must not compile");
        assert!(message.starts_with("cannot compile to WebAssembly"), "{message}");
        let no_ui = load("noui", "fn main() { 1 }");
        assert!(crate::wasm_app::compile(&no_ui).err().unwrap().contains("no `ui fn`"));
    }

    #[test]
    fn embedded_source_cannot_close_the_script_element() {
        let html = standalone_html(
            "fn s() = \"<script src='x'></script>\"\nui fn App() {}",
            "App",
            &tint_ast::AppMeta::default(),
            &InlineAssets::default(),
        );
        assert!(!html.contains("'x'></script>"));
        assert!(html.contains("<\\/script>"));
    }
}
