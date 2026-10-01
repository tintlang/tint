//! `app { rs::"./native.rs" }`: builds a small cargo project that links the
//! Rust files (with `tint` and this CLI) and runs the same command through it.
//! It lives in `.tint/native/` next to the entry file and is rebuilt by cargo
//! incrementally.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ACTIVE_VAR: &str = "TINT_NATIVE_RUNNER";

/// True inside the generated runner (so it does not build itself again).
pub(crate) fn active() -> bool {
    std::env::var_os(ACTIVE_VAR).is_some()
}

/// Where the `tint` and `tint-cli` crates are: `TINT_CRATES`, else the
/// checkout this binary was built from.
pub(crate) fn crates_dir() -> PathBuf {
    std::env::var_os("TINT_CRATES")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf())
}

fn fail(message: String) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}

pub(crate) fn write_if_changed(path: &Path, text: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() != Some(text) {
        fs::write(path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn exec(entry: &str, rs_files: &[String]) -> ! {
    let entry_abs = fs::canonicalize(entry).unwrap_or_else(|e| fail(format!("cannot read '{entry}': {e}")));
    let base = entry_abs.parent().unwrap_or(Path::new("."));
    let dir = base.join(".tint").join("native");
    if let Err(error) = fs::create_dir_all(dir.join("src")) {
        fail(format!("cannot create {}: {error}", dir.display()));
    }

    let crates = crates_dir();
    for needed in ["tint", "tint-cli"] {
        if !crates.join(needed).join("Cargo.toml").exists() {
            fail(format!(
                "cannot find the `{needed}` crate in {} (set TINT_CRATES to the folder holding the tint crates)",
                crates.display()
            ));
        }
    }

    let mut modules = String::new();
    for (index, file) in rs_files.iter().enumerate() {
        let path = fs::canonicalize(base.join(file))
            .unwrap_or_else(|e| fail(format!("cannot read '{file}' (rs::): {e}")));
        modules.push_str(&format!("#[path = {:?}]\nmod user{index};\n", path.to_string_lossy()));
    }

    let manifest = format!(
        "[package]\nname = \"tint-native-runner\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n\
         [workspace]\n\n\
         [dependencies]\ntint = {{ path = {:?} }}\ntint-cli = {{ path = {:?}, default-features = false }}\n",
        crates.join("tint").to_string_lossy(),
        crates.join("tint-cli").to_string_lossy(),
    );
    let main = format!(
        "#![allow(dead_code)]\n{modules}\nfn main() {{\n    \
         tint_cli::main_with(std::env::args().collect(), tint::registered_natives());\n}}\n"
    );
    write_if_changed(&dir.join("Cargo.toml"), &manifest).unwrap_or_else(|e| fail(e));
    write_if_changed(&dir.join("src").join("main.rs"), &main).unwrap_or_else(|e| fail(e));
    write_if_changed(&base.join(".tint").join(".gitignore"), "*\n").unwrap_or_else(|e| fail(e));

    let forwarded: Vec<String> = std::env::args().skip(1).collect();
    let status = Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .arg("--")
        .args(&forwarded)
        .env(ACTIVE_VAR, "1")
        .status()
        .unwrap_or_else(|e| fail(format!("cannot run cargo (is Rust installed?): {e}")));
    std::process::exit(status.code().unwrap_or(1));
}
