// Proves `TintVM::register_native` -- direct Rust interop: register an
// ordinary Rust closure under a name, and `.tn` source calls it directly, no
// Rust parsing or borrow-checking of Tint code involved anywhere.
//
// Three call sites are exercised, matching the three places this is
// actually reachable in the current runtime:
//   - `TintVM::call_fn`, the direct/top-level invocation an embedder uses
//     (this is exactly what `tint run file.tn <name>` calls with `<name>`
//     as `fn_name` -- if `<name>` is a registered native rather than a
//     Tint `fn`, it's called with no Tint source involved at all).
//   - `TintVM::call_user_fn`, the tree-walking dispatch used for
//     click/hover handlers (see `UiSession::dispatch`) and other
//     tree-walked bodies -- a *nested* call from inside a Tint `fn`'s body
//     to a native fn now resolves correctly there (see the `Expr::Call`
//     fix in tint-evaluator/src/evaluator/expr.rs).
//   - `TintVM::call_fn`'s IR branch: a plain `fn`, invoked the normal
//     top-level way (so it runs through `IrVM`, not the tree-walking
//     evaluator), whose OWN body calls a native fn. This used to be a real
//     gap: `tint-ir` (where `IrVM` lives) has no dependency on
//     `tint-runtime`'s `native_fns` table at all, so a name that resolved
//     to nothing among the IR-compiled `.tn` functions just returned
//     `None` and the call silently became `Unit`, no matter what was
//     registered. Fixed by giving `IrVM` a `native_call` callback slot
//     (`set_native_call`) that `call_named_function` falls back to when no
//     IR-compiled function matches the name, and having `call_fn`'s IR
//     branch install one backed by `TintVM::native_fns` before running the
//     `IrVM`. `native_fns` had to move from a plain `HashMap` to
//     `Rc<RefCell<HashMap<...>>>` so that callback -- which must be
//     `'static`, since `IrVM` holds it with no lifetime parameter -- could
//     `Rc::clone` a handle into the SAME table instead of borrowing `&self`
//     (which would fight the `&mut self` receiver of `call_fn`). See
//     `native_fn_reachable_from_a_plain_fn_called_via_the_ir_path` below.

use tint_evaluator::value::Value;
use tint_evaluator::EvalHost;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_runtime::vm::TintVM;

fn parse_and_run(code: &str, vm: &mut TintVM) {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().expect("source should parse");
    vm.run_program(&program);
}

include!("direct.rs");
include!("ir.rs");
include!("host_builtins.rs");
