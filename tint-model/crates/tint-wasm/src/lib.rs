// tint-wasm/src/lib.rs
//
// Step B of the sandbox plan: a thin wasm-bindgen bridge exposing the
// SAME lexer -> parser -> TintVM pipeline tint-cli drives from the
// command line, but callable from JS in a browser. This crate holds no
// language logic of its own -- it only translates between JS and the
// existing tint-* crates, mirroring tint-cli/src/main.rs's cmd_check/
// cmd_run.
//
// Two entry points:
//   check(source)       -- lex + parse only, report ok/error + span.
//   run(source, entry)  -- parse, compile to IR, run `entry` (no args),
//                           report the result value, any error, and
//                           whatever the program printed via print()/dbg()
//                           (captured through tint_evaluator::output,
//                           since there is no real stdout in a browser).
//
// Both return a JS object (via serde-wasm-bindgen) rather than throwing,
// so the sandbox UI can always render *something* -- a value, an error
// with a span to highlight, or both.

use serde::Serialize;
use wasm_bindgen::prelude::*;

mod dom;
pub use dom::DomSession;

use tint_ast::{Item, Program, Span};
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

include!("api/core.rs");
include!("api/ui.rs");
include!("api/repl.rs");
include!("api/session.rs");
