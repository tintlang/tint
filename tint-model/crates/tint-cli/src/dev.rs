//! `tint dev <file.tn> [ui_fn] [--port N]`: serves the app with no HTML, JS or
//! bundler of its own and reloads the page whenever a reachable `.tn` file
//! changes. The page is the same self-contained shell `tint build` writes, and
//! every path serves it (single-page-app fallback).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use crate::build::{infer_entry, read_inline_assets, standalone_html};
use crate::module_loader;
use crate::support::{require_arg, semantic_check};

const RELOAD_SCRIPT: &str = r#"<script>
(function () {
    var last = null;
    setInterval(function () {
        fetch('/__tint_version', { cache: 'no-store' })
            .then(function (r) { return r.text(); })
            .then(function (v) { if (last !== null && v !== last) location.reload(); last = v; })
            .catch(function () {});
    }, 400);
})();
</script>"#;

pub(crate) fn command(args: &[String]) {
    let usage = "tint dev <file.tn> [ui_fn] [--port N]";
    let path = require_arg(args.get(2), usage).to_string();
    let mut entry = None;
    let mut port = 5173u16;
    let mut index = 3;
    while index < args.len() {
        if args[index] == "--port" {
            port = args
                .get(index + 1)
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(|| {
                    eprintln!("usage: {}", usage);
                    std::process::exit(1);
                });
            index += 2;
        } else {
            entry = Some(args[index].clone());
            index += 1;
        }
    }

    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|error| {
        eprintln!("error: cannot listen on port {}: {}", port, error);
        std::process::exit(1);
    });
    println!("tint dev: http://localhost:{}  (serving {})", port, path);

    for stream in listener.incoming().flatten() {
        let (path, entry) = (path.clone(), entry.clone());
        thread::spawn(move || handle(stream, &path, entry.as_deref()));
    }
}

fn handle(mut stream: TcpStream, path: &str, entry: Option<&str>) {
    let mut buffer = [0u8; 2048];
    let Ok(read) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..read]);
    let target = request.split_whitespace().nth(1).unwrap_or("/");

    let (content_type, body) = match target {
        "/__tint_version" => ("text/plain", version(path)),
        "/favicon.ico" => {
            respond(&mut stream, "404 Not Found", "text/plain", "not found");
            return;
        }
        // Every other path gets the app, so `app { router::on }` routes
        // (`/about`) survive a reload.
        _ => ("text/html; charset=utf-8", page(path, entry)),
    };
    respond(&mut stream, "200 OK", content_type, &body);
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Changes whenever any reachable source (or an error) changes.
fn version(path: &str) -> String {
    let mut hasher = DefaultHasher::new();
    // A restarted server (rebuilt binary) must also reload open pages.
    static STARTED: std::sync::OnceLock<std::time::SystemTime> = std::sync::OnceLock::new();
    STARTED.get_or_init(std::time::SystemTime::now).hash(&mut hasher);
    match module_loader::try_load(path) {
        Ok(loaded) => {
            loaded.all_sources.hash(&mut hasher);
            if let Ok(inline) = read_inline_assets(path, &loaded.program.app_meta()) {
                inline.js.hash(&mut hasher);
                inline.wasm.as_ref().map(|w| w.wasm_gz.hash(&mut hasher));
                inline.css.hash(&mut hasher);
            }
        }
        Err(error) => error.hash(&mut hasher),
    }
    hasher.finish().to_string()
}

fn page(path: &str, entry: Option<&str>) -> String {
    let html = match module_loader::try_load(path) {
        Ok(loaded) => {
            for error in &semantic_check(&loaded) {
                eprintln!("warning: semantic:");
                crate::support::report_semantic_error(&loaded.entry_source, error);
            }
            let entry = entry.map_or_else(|| infer_entry(&loaded.entry_source), str::to_owned);
            let meta = loaded.program.app_meta();
            match read_inline_assets(path, &meta) {
                Ok(inline) => {
                    standalone_html(&loaded.all_sources.join("\n\n"), &entry, &meta, &inline)
                }
                Err(error) => {
                    eprintln!("{}", error);
                    format!(
                        "<!DOCTYPE html><meta charset=\"utf-8\"><body style=\"font-family:monospace\"><pre>{}</pre></body>",
                        error.replace('&', "&amp;").replace('<', "&lt;")
                    )
                }
            }
        }
        Err(error) => {
            eprintln!("{}", error);
            format!(
                "<!DOCTYPE html><meta charset=\"utf-8\"><body style=\"font-family:monospace\"><pre>{}</pre></body>",
                error.replace('&', "&amp;").replace('<', "&lt;")
            )
        }
    };
    match html.rfind("</body>") {
        Some(at) => format!("{}{}{}", &html[..at], RELOAD_SCRIPT, &html[at..]),
        None => html + RELOAD_SCRIPT,
    }
}
