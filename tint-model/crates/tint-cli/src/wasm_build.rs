//! `app { rs::"./native.rs" }` for `tint build` / `tint dev`: compiles the Rust
//! files to a small wasm module (with `wasm-pack`) whose `#[tint::export]`
//! functions the page registers as natives. Built in `.tint/wasm/` next to the
//! entry file; cargo rebuilds it incrementally.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::native_runner::{crates_dir, write_if_changed};

pub(crate) struct WasmAssets {
    /// wasm-bindgen's ES module glue (`--target web`).
    pub glue: String,
    /// The module, gzipped.
    pub wasm_gz: Vec<u8>,
}

static CACHE: Mutex<Option<(u64, Arc<WasmAssets>)>> = Mutex::new(None);

pub(crate) fn build(entry_path: &str, rs_files: &[String]) -> Result<Arc<WasmAssets>, String> {
    let entry = fs::canonicalize(entry_path).map_err(|e| format!("cannot read '{entry_path}': {e}"))?;
    let base = entry.parent().unwrap_or(Path::new("."));

    let mut sources = Vec::new();
    let mut hasher = DefaultHasher::new();
    for file in rs_files {
        let path = fs::canonicalize(base.join(file)).map_err(|e| format!("cannot read '{file}' (rs::): {e}"))?;
        fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?.hash(&mut hasher);
        path.hash(&mut hasher);
        sources.push(path);
    }
    let key = hasher.finish();
    if let Some((cached, assets)) = CACHE.lock().unwrap().as_ref() {
        if *cached == key {
            return Ok(Arc::clone(assets));
        }
    }

    let crates = crates_dir();
    if !crates.join("tint").join("Cargo.toml").exists() {
        return Err(format!(
            "cannot find the `tint` crate in {} (set TINT_CRATES to the folder holding the tint crates)",
            crates.display()
        ));
    }
    let dir = base.join(".tint").join("wasm");
    fs::create_dir_all(dir.join("src")).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    let manifest = format!(
        "[package]\nname = \"tint_native_wasm\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n\
         [lib]\ncrate-type = [\"cdylib\"]\n\n\
         [workspace]\n\n\
         [dependencies]\ntint = {{ path = {:?}, default-features = false }}\nwasm-bindgen = \"0.2\"\n\n\
         [profile.release]\nopt-level = \"z\"\nlto = true\ncodegen-units = 1\npanic = \"abort\"\nstrip = true\n\n\
         [package.metadata.wasm-pack.profile.release]\nwasm-opt = false\n",
        crates.join("tint").to_string_lossy(),
    );
    let mut lib = String::from("#![allow(dead_code)]\n");
    for (index, path) in sources.iter().enumerate() {
        lib.push_str(&format!("#[path = {:?}]\nmod user{index};\n", path.to_string_lossy()));
    }
    write_if_changed(&dir.join("Cargo.toml"), &manifest)?;
    write_if_changed(&dir.join("src").join("lib.rs"), &lib)?;
    write_if_changed(&base.join(".tint").join(".gitignore"), "*\n")?;

    eprintln!("compiling rs:: files to wasm (wasm-pack)...");
    let status = Command::new("wasm-pack")
        .arg("build")
        .arg(&dir)
        .args(["--release", "--target", "web", "--out-dir", "pkg", "--no-typescript"])
        .status()
        .map_err(|e| format!("cannot run wasm-pack ({e}); install it with `cargo install wasm-pack`"))?;
    if !status.success() {
        return Err("wasm-pack failed (see the output above)".to_string());
    }

    let glue = fs::read_to_string(dir.join("pkg").join("tint_native_wasm.js"))
        .map_err(|e| format!("missing wasm-pack output: {e}"))?;
    let wasm = fs::read(dir.join("pkg").join("tint_native_wasm_bg.wasm"))
        .map_err(|e| format!("missing wasm-pack output: {e}"))?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&wasm).map_err(|e| e.to_string())?;
    let wasm_gz = encoder.finish().map_err(|e| e.to_string())?;
    eprintln!("rs:: wasm: {} KB, {} KB gzipped", wasm.len() / 1024, wasm_gz.len() / 1024);

    let assets = Arc::new(WasmAssets { glue, wasm_gz });
    *CACHE.lock().unwrap() = Some((key, Arc::clone(&assets)));
    Ok(assets)
}
