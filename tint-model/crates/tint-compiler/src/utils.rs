use std::path::{Path, PathBuf};

use tint_ast::UseDecl;

use crate::SourceDatabase;

pub(super) fn source_text(sources: &SourceDatabase, id: crate::SourceId) -> Result<String, String> {
    sources
        .source(id)
        .map(|source| source.text.clone())
        .ok_or_else(|| "source disappeared".into())
}

pub(super) fn append(path: &[String], name: &str) -> Vec<String> {
    let mut result = path.to_vec();
    result.push(name.to_string());
    result
}

pub(super) fn path_string(path: &[String]) -> String {
    if path.is_empty() {
        "<entry file>".into()
    } else {
        path.join("::")
    }
}

pub(super) fn use_string(declaration: &UseDecl) -> String {
    let base = declaration.path.join("::");
    if declaration.wildcard {
        format!("{}::*", base)
    } else if let Some(alias) = &declaration.alias {
        format!("{} as {}", base, alias)
    } else {
        base
    }
}

pub(super) fn canonical_or_absolute(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    })
}
