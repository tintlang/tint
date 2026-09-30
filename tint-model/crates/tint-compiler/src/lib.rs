//! Shared compiler input and parse state.
//!
//! The CLI used to own file loading and parsing.  This small database is the
//! first compiler-facing layer that is independent of the CLI, runtime, and
//! filesystem process-exit policy.  The semantic checker and the future
//! language server can build on the same source IDs and parsed programs.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tint_ast::{AttributeList, Item, Program, Span, UiFnDecl};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::{error::ParserError, Parser};

pub mod resolver;

/// Parses one source buffer without adding it to a database.
pub fn parse_source(text: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(text));
    Parser::new(tokens).parse_program()
}

/// Parses either a regular Tint program or a source-import UI fragment.
///
/// Vite expands files such as `topbar.tn` into the body of a `ui fn`, so
/// those files are valid Tint source even though they do not begin with
/// `fn`, `ui fn`, `struct`, or another top-level declaration.  The language
/// server needs to understand that same file shape when the fragment itself
/// is open in the editor.
pub fn parse_source_for_analyzer(text: &str) -> Result<Program, ParserError> {
    match parse_source(text) {
        Ok(program) => Ok(program),
        Err(original) => {
            let tokens = collect_tokens(&mut Lexer::new(text));
            match Parser::new(tokens).parse_ui_fragment() {
                Ok(body) => Ok(Program {
                    globals: AttributeList::empty(),
                    items: vec![Item::UiFn(UiFnDecl {
                        attributes: AttributeList::empty(),
                        name: "__tint_fragment".into(),
                        params: Vec::new(),
                        state: Vec::new(),
                        body,
                        span: Span::dummy(),
                    })],
                }),
                // The first error came from trying to parse a fragment as a
                // top-level program. If it really is a fragment but malformed
                // (for example, a missing `}`), the fragment parser's error
                // points at the actual failure and must win.
                Err(fragment_error) => Err(
                    if matches!(
                        &original,
                        ParserError::Message { msg, .. }
                            if msg.contains("expected fn/ui/struct/enum")
                    ) {
                        fragment_error
                    } else {
                        original
                    },
                ),
            }
        }
    }
}

/// Expands the source-level `.tn` imports used by the Vite loader.  Imports
/// are textual by design: a file such as `topbar.tn` contributes UI nodes to
/// the surrounding `ui fn`, while `tokens.tn` contributes theme declarations.
/// Keeping this in the compiler makes the resolver and language server use
/// the same project view as the runtime.
pub fn expand_source_imports<P: AsRef<Path>>(
    entry_path: P,
    overlays: &HashMap<PathBuf, String>,
) -> Result<String, String> {
    let mut active = Vec::new();
    expand_source_file(entry_path.as_ref(), overlays, &mut active)
}

fn expand_source_file(
    path: &Path,
    overlays: &HashMap<PathBuf, String>,
    active: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let normalized = normalize_path(path);
    if let Some(index) = active.iter().position(|item| item == &normalized) {
        let mut cycle = active[index..].to_vec();
        cycle.push(normalized.clone());
        return Err(format!(
            "Tint import cycle: {}",
            cycle
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ")
        ));
    }

    let source = if let Some(text) = overlays.get(&normalized) {
        text.clone()
    } else {
        fs::read_to_string(&normalized)
            .map_err(|error| format!("error reading '{}': {}", normalized.display(), error))?
    };

    active.push(normalized.clone());
    let mut expanded = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        if let Some(specifier) = source_import_specifier(line) {
            let child = normalized
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(specifier);
            expanded.push_str(&expand_source_file(&child, overlays, active)?);
        } else if line.contains("include_str(\"") {
            let dir = normalized.parent().unwrap_or_else(|| Path::new("."));
            expanded.push_str(&expand_include_str(line, dir)?);
        } else {
            expanded.push_str(line);
        }
    }
    if !source.ends_with('\n') {
        // `split_inclusive` still returns the final unterminated line, so this
        // branch intentionally documents that no delimiter is added.
    }
    active.pop();
    Ok(expanded)
}

/// Replaces every `include_str("relative/path")` on `line` with the file's
/// text as a Tint string literal, so a program can embed sample source.
fn expand_include_str(line: &str, dir: &Path) -> Result<String, String> {
    const MARKER: &str = "include_str(\"";
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find(MARKER) {
        out.push_str(&rest[..at]);
        let after = &rest[at + MARKER.len()..];
        let Some(end) = after.find("\")") else {
            return Err("include_str: missing closing `\")`".to_string());
        };
        let target = normalize_path(&dir.join(&after[..end]));
        let text = fs::read_to_string(&target).map_err(|error| {
            format!(
                "include_str: error reading '{}': {}",
                target.display(),
                error
            )
        })?;
        out.push('"');
        for c in text.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                '{' => out.push_str("\\{"),
                '}' => out.push_str("\\}"),
                other => out.push(other),
            }
        }
        out.push('"');
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn source_import_specifier(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = line.strip_prefix("import \"")?;
    let end = rest.find('"')?;
    let specifier = &rest[..end];
    let tail = rest[end + 1..].trim();
    if !specifier.ends_with(".tn") || !(tail.is_empty() || tail == ";") {
        return None;
    }
    Some(specifier)
}

/// Stable handle for a source file inside a [`SourceDatabase`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(usize);

/// Source text and its canonical path.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: SourceId,
    pub path: PathBuf,
    pub text: String,
}

/// In-memory compiler inputs with successful parse results cached by source ID.
///
/// This deliberately does not perform module resolution yet.  It gives the
/// resolver a stable home in the next step: paths and source contents are
/// owned here, while resolution can later add a graph and dependency cache
/// without changing the parser or CLI API.
#[derive(Debug, Default)]
pub struct SourceDatabase {
    files: Vec<SourceFile>,
    by_path: HashMap<PathBuf, SourceId>,
    parsed: HashMap<SourceId, Program>,
}

impl SourceDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces source text at `path`, invalidating its parse cache.
    pub fn set_source<P: AsRef<Path>>(&mut self, path: P, text: impl Into<String>) -> SourceId {
        let path = normalize_path(path.as_ref());
        if let Some(&id) = self.by_path.get(&path) {
            self.files[id.0].text = text.into();
            self.parsed.remove(&id);
            return id;
        }

        let id = SourceId(self.files.len());
        self.files.push(SourceFile {
            id,
            path: path.clone(),
            text: text.into(),
        });
        self.by_path.insert(path, id);
        id
    }

    /// Reads a source file and inserts it into the database.
    pub fn load_path<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<SourceId> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)?;
        Ok(self.set_source(path, text))
    }

    pub fn source(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.0)
    }

    pub fn source_id<P: AsRef<Path>>(&self, path: P) -> Option<SourceId> {
        self.by_path.get(&normalize_path(path.as_ref())).copied()
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
    }

    /// Parses a source, reusing the previous successful result when possible.
    pub fn parse(&mut self, id: SourceId) -> Result<Program, ParserError> {
        if let Some(program) = self.parsed.get(&id) {
            return Ok(program.clone());
        }

        let source = self.source(id).ok_or_else(|| ParserError::Message {
            msg: format!("unknown source id {}", id.0),
            span: tint_ast::Span::dummy(),
        })?;
        let program = parse_source(&source.text)?;
        self.parsed.insert(id, program.clone());
        Ok(program)
    }

    pub fn invalidate(&mut self, id: SourceId) {
        self.parsed.remove(&id);
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(path) = path.canonicalize() {
        return path;
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_successful_parse_and_invalidates_on_edit() {
        let mut db = SourceDatabase::new();
        let id = db.set_source("main.tn", "fn main() = 1");
        assert_eq!(db.parse(id).unwrap().items.len(), 1);
        assert_eq!(db.parse(id).unwrap().items.len(), 1);

        db.set_source("main.tn", "fn main() = 2");
        assert_eq!(db.parse(id).unwrap().items.len(), 1);
    }

    #[test]
    fn reuses_source_id_for_the_same_path() {
        let mut db = SourceDatabase::new();
        let first = db.set_source("main.tn", "fn main() = 1");
        let second = db.set_source("main.tn", "fn main() = 2");
        assert_eq!(first, second);
        assert_eq!(db.sources().count(), 1);
        assert_eq!(db.source(first).unwrap().text, "fn main() = 2");
    }

    #[test]
    fn parses_source_import_ui_fragments() {
        let program = parse_source_for_analyzer("Nav { Logo { route||\"/\" \"tint\" } }")
            .expect("a source-import UI fragment should parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn reports_the_real_error_for_an_unclosed_ui_fragment() {
        let error = parse_source_for_analyzer("Nav { Logo { route||\"/\" \"tint\" }")
            .expect_err("the missing closing brace must be rejected");
        assert!(matches!(
            error,
            tint_parser::error::ParserError::Unexpected {
                expected: tint_lexer::TokenKind::RBrace,
                found: tint_lexer::TokenKind::Eof,
                ..
            }
        ));
    }

    #[test]
    fn include_str_embeds_a_file_with_braces_and_quotes_as_one_literal() {
        let dir = std::env::temp_dir().join(format!("tint-include-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("sample.tn"), "ui fn A() { \"x {y}\" }\n").unwrap();
        std::fs::write(
            dir.join("main.tn"),
            "fn sample() = include_str(\"sample.tn\")\n",
        )
        .unwrap();
        let expanded = expand_source_imports(dir.join("main.tn"), &HashMap::new()).unwrap();
        assert_eq!(
            expanded,
            "fn sample() = \"ui fn A() \\{ \\\"x \\{y\\}\\\" \\}\\n\"\n"
        );
        let mut db = SourceDatabase::new();
        let id = db.set_source("main.tn", expanded);
        assert_eq!(db.parse(id).unwrap().items.len(), 1);
    }
}
