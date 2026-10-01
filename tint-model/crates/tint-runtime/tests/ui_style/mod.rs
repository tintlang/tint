// Proves that a UI node's `key::value` modifiers -- including a nested
// `hover::{ ... }` -- actually resolve into CSS style data on the built
// UiTree, instead of hover being something only the sandbox's own
// hardcoded, tag-name-based CSS fakes (see sandbox/src/components/
// UiPreviewNode.svelte's TAG_DEFAULTS comment for what that used to look
// like). This exercises tint-runtime/src/ui/style.rs end to end, through
// the real lexer -> parser -> UiBuilder pipeline.

use tint_ast::Item;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::ui::builder::UiBuilder;
use tint_runtime::vm::TintVM;

fn parse_ui_fn(code: &str, name: &str) -> tint_ast::UiFnDecl {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("parse failed");

    for item in program.items {
        if let Item::UiFn(f) = item {
            if f.name == name {
                return f;
            }
        }
    }

    panic!("ui fn `{}` not found", name);
}

include!("base.rs");
include!("spacing.rs");
include!("layout.rs");
include!("components.rs");
include!("motion.rs");
