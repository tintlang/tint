// A hand-rolled StreamLanguage tokenizer for basic Tint syntax
// highlighting in the sandbox's CodeMirror editor -- not a real
// grammar/parser (no incremental/lossless parse tree, no
// syntax-error-aware highlighting), just enough of a line-by-line scanner
// to color keywords/strings/numbers/comments/operators. Keyword list is
// copied directly from tint-lexer/src/lexer/literals.rs's keyword match arms, so
// it stays in sync by inspection rather than by import (the lexer crate
// isn't and shouldn't be pulled into a JS bundle).
import { StreamLanguage } from "@codemirror/language";

const KEYWORDS = new Set([
  "fn", "ui", "true", "false", "_",
  "return", "let", "if", "else", "match", "for", "in", "where", "while",
  "loop", "break", "continue", "async", "await", "try",
  "enum", "struct", "impl", "self", "space",
  "borrow", "immut", "move", "clone",
  "state", "signal", "computed", "tint2d",
  "module", "mod", "export", "use",
  "map", "type", "kernel",
]);

// Tint-lexer treats these as TRUE keywords but they read like builtin
// literals/values in practice -- kept in the same highlight class as
// true/false rather than the general keyword color, purely cosmetic.
const ATOM_WORDS = new Set(["true", "false", "self", "_"]);

function tokenTint(stream, state) {
  if (state.inBlockComment) {
    // Tint's lexer only has `//` line comments today, but this sandbox's
    // highlighter is intentionally forward-tolerant of `/* */` in case
    // that's ever added -- unrecognized syntax just won't highlight.
    if (stream.match(/^.*?\*\//)) {
      state.inBlockComment = false;
    } else {
      stream.skipToEnd();
    }
    return "comment";
  }

  if (stream.eatSpace()) return null;

  if (stream.match("//")) {
    stream.skipToEnd();
    return "comment";
  }

  if (stream.match("/*")) {
    state.inBlockComment = true;
    return "comment";
  }

  // Hex color literal (#RGB / #RRGGBB / #RRGGBBAA), a real Tint-lexer
  // token shape (see lexer.rs's hex-color comment) -- highlighted before
  // the generic operator fallback would otherwise eat the `#`.
  if (stream.match(/^#[0-9a-fA-F]{3,8}\b/)) return "atom";

  if (stream.match(/^"(?:[^"\\]|\\.)*"/)) return "string";

  if (stream.match(/^-?\d+(\.\d+)?/)) return "number";

  if (stream.match(/^[A-Za-z_][A-Za-z0-9_-]*/)) {
    const word = stream.current();
    if (KEYWORDS.has(word)) {
      return ATOM_WORDS.has(word) ? "atom" : "keyword";
    }
    // Heuristic only (no real name resolution here): `name(` looks like a
    // call, `Name` (capitalized) looks like a type/struct/UI tag.
    if (stream.match(/^\s*\(/, false)) return "variableName.function";
    if (/^[A-Z]/.test(word)) return "typeName";
    return "variableName";
  }

  // `attr||value` (UiAttribute) and `path.segment::value` (UiModifier)
  // are real, distinctive Tint syntax -- worth their own token class
  // rather than falling into the generic operator bucket.
  if (stream.match("||")) return "operatorKeyword";
  if (stream.match("::")) return "operatorKeyword";

  if (stream.match(/^(==|!=|<=|>=|&&|\|\||->|=>|\.\.)/)) return "operator";
  if (stream.match(/^[+\-*/%=<>!&|^~]/)) return "operator";
  if (stream.match(/^[(){}\[\],;.:]/)) return "punctuation";

  stream.next();
  return null;
}

export function tintLanguage() {
  return StreamLanguage.define({
    name: "tint",
    startState: () => ({ inBlockComment: false }),
    token: tokenTint,
    languageData: {
      commentTokens: { line: "//", block: { open: "/*", close: "*/" } },
    },
  });
}
