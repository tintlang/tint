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

#![cfg_attr(not(feature = "interpreter"), allow(dead_code))]

#[cfg(feature = "sandbox-api")]
use serde::Serialize;
#[cfg(feature = "sandbox-api")]
use wasm_bindgen::prelude::*;

mod dom;
pub use dom::DomSession;

// The sandbox's own entry points (check/run/REPL/render a tree as data). A
// standalone page only needs `DomSession`, so it is built without them.
#[cfg(feature = "sandbox-api")]
use tint_ast::{Item, Program, Span};
#[cfg(feature = "sandbox-api")]
use tint_lexer::{collect_tokens, Lexer};
#[cfg(feature = "sandbox-api")]
use tint_parser::error::ParserError;
#[cfg(feature = "sandbox-api")]
use tint_parser::Parser;
#[cfg(feature = "sandbox-api")]
use tint_runtime::vm::TintVM;

#[cfg(feature = "sandbox-api")]
include!("api/core.rs");
#[cfg(feature = "sandbox-api")]
include!("api/ui.rs");
#[cfg(feature = "sandbox-api")]
include!("api/repl.rs");
#[cfg(feature = "sandbox-api")]
include!("api/session.rs");
#[cfg(feature = "bytecode")]
include!("api/bytecode.rs");
