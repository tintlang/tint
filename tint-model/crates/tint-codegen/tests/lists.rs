use tint_ir::typed::lower_program;
use tint_lexer::{collect_tokens, Lexer};
use tint_parser::Parser;
use tint_semantics::prelude::CheckerContext;
use tint_semantics::SemanticChecker;

/// Compiles `source` natively and returns what `main` returns, rendered.
fn run(source: &str) -> String {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let program = Parser::new(tokens).parse_program().expect("parse");
    let ctx = CheckerContext { strict: true, ..Default::default() };
    let (errors, model) = SemanticChecker::new(ctx).check_with_model(&program);
    assert!(errors.is_empty(), "{errors:?}");
    let lowered = lower_program(&program, &model);
    assert!(lowered.errors.is_empty(), "{:?}", lowered.errors);
    tint_codegen::compile(lowered.module)
        .expect("native backend accepts the program")
        .run_main()
        .expect("runs")
}

#[test]
fn a_copy_of_a_list_is_independent() {
    let out = run("fn main() {
        let a = [1, 2, 3]
        let mut b = a
        b[0] = 9
        a[0] * 10 + b[0]
    }");
    assert_eq!(out, "19");
}

#[test]
fn a_callee_cannot_change_the_callers_list() {
    let out = run("fn bump(mut xs: Vec<number>) -> number {
        xs[0] = xs[0] + 100
        xs[0]
    }
    fn main() {
        let a = [1, 2, 3]
        let r = bump(a)
        r * 1000 + a[0]
    }");
    assert_eq!(out, "101001");
}

#[test]
fn push_grows_and_len_counts() {
    let out = run("fn main() {
        let mut xs = []
        let mut i = 0
        while i < 1000 {
            xs.push(i)
            i = i + 1
        }
        xs.len() + xs[999]
    }");
    assert_eq!(out, "1999");
}

#[test]
fn bool_and_i32_elements_keep_their_width() {
    let out = run("fn main() {
        let mut flags = [true, false, true]
        flags[1] = true
        let mut n: Vec<i32> = [0 - 5, 7]
        n[0] = n[0] - 1
        if flags[1] { n[0] + n[1] } else { 0 }
    }");
    assert_eq!(out, "1i32");
}
