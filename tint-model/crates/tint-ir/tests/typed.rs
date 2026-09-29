//! Typed IR: lowering, verifier and reference interpreter.

use tint_ir::typed::verify::verify;
use tint_ir::typed::{interp::render, lower_program, Interp, Lowered};
use tint_ir::typed::ir::{BlockId, Term};
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

fn lower(source: &str) -> Lowered {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    assert!(errors.is_empty(), "type errors: {errors:?}");
    lower_program(&program, &model)
}

fn run(source: &str) -> String {
    let lowered = lower(source);
    assert!(lowered.errors.is_empty(), "lowering errors: {:?}", lowered.errors);
    assert_eq!(verify(&lowered.module), Vec::<String>::new());
    let mut interp = Interp::new(&lowered.module);
    let (value, ty) = interp.run("main", vec![]).expect("run");
    render(&lowered.module, ty, &value)
}

#[test]
fn closures_capture_by_value() {
    let src = "fn main() -> number {\n let k = 10\n let add = |x| x + k\n return add(5)\n}";
    assert_eq!(run(src), "15");
}

#[test]
fn inout_method_updates_receiver() {
    let src = "struct C { n: number }\nimpl C { fn bump(self) { self.n = self.n + 1 } }\nfn main() -> number {\n let mut c = C { n: 1 }\n c.bump()\n c.bump()\n return c.n\n}";
    assert_eq!(run(src), "3");
}

#[test]
fn nested_place_assignment() {
    let src = "fn main() -> number {\n let mut xs = [1, 2, 3]\n xs[1] = 20\n return xs[1]\n}";
    assert_eq!(run(src), "20");
}

#[test]
fn question_mark_propagates_err() {
    let src = "fn f(x: number) -> Result<number, string> {\n if x > 0 { return Ok(x) }\n return Err(\"neg\")\n}\nfn g(x: number) -> Result<number, string> {\n let v = f(x)?\n return Ok(v + 1)\n}\nfn main() -> string {\n match g(0) {\n  Ok(v) => \"ok\",\n  Err(e) => e,\n }\n}";
    assert_eq!(run(src), "\"neg\"");
}

#[test]
fn dump_lists_functions_and_blocks() {
    let lowered = lower("fn main() -> number { return 1 + 2 }");
    let text = lowered.module.to_string();
    assert!(text.contains("main"), "{text}");
    assert!(text.contains("ret"), "{text}");
}

#[test]
fn verifier_rejects_dangling_jump() {
    let mut lowered = lower("fn main() -> number { return 1 }");
    let id = lowered.module.functions["main"];
    lowered.module.funcs[id.0 as usize].blocks[0].term = Term::Jump(BlockId(99));
    assert!(!verify(&lowered.module).is_empty());
}

#[test]
fn verifier_rejects_bad_return_register() {
    let mut lowered = lower("fn main() -> number { return 1 }");
    let id = lowered.module.functions["main"];
    let f = &mut lowered.module.funcs[id.0 as usize];
    f.blocks[0].term = Term::Return(tint_ir::typed::ir::Reg(9999));
    assert!(!verify(&lowered.module).is_empty());
}

#[test]
fn method_without_self_is_a_lowering_error() {
    let lowered = lower("struct C { n: number }\nimpl C { fn make() -> number { return 1 } }\nfn main() -> number { return 0 }");
    assert!(lowered.errors.iter().any(|e| e.message.contains("self")), "{:?}", lowered.errors);
}

#[test]
fn examples_lowering_is_measured() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (mut ok, mut total) = (0, 0);
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tn") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = collect_tokens(&mut Lexer::new(&source));
        let Ok(program) = Parser::new(tokens).parse_program() else { continue };
        let (errors, model) = SemanticChecker::new(CheckerContext::default()).check_with_model(&program);
        if !errors.is_empty() {
            continue;
        }
        total += 1;
        let lowered = lower_program(&program, &model);
        assert_eq!(verify(&lowered.module), Vec::<String>::new(), "{}", path.display());
        if lowered.errors.is_empty() {
            ok += 1;
        }
    }
    eprintln!("examples lowered cleanly: {ok}/{total}");
    assert!(total > 0);
}
