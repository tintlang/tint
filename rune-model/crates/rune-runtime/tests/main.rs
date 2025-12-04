#[test]
fn test_rune_capabilities() {
    use rune_parser::Parser;
    use rune_lexer::{Lexer, collect_tokens, Token};
    use rune_runtime::vm::RuneVM;
    use rune_evaluator::EvalHost;

    use rune_parser::error::ParserError;

    println!("\n\n============================= RUNE TEST =================================");
    println!("→ STEP 1: Loading source code\n");

    let code = r#"
fn AllFeatures() {
    let p = (10, 20, 30);
    let (x, y, z) = p;
    let s1 = x + y + z;

    let arr = [x, y, z, s1];
    let a0 = arr[0];
    let a3 = arr[3];

    let u = User {
        id{a0},
        name{"Rune"},
        age{99},
    };

    let user2 = user {
        age{user.age + 1},
        name{"Nornse"}
    }

    let t = (1, 2, 3);
    let res = match t {
        (1, b, 3) => b * 10,
        _ => 0
    };

    let who = match user2 {
        User { id, name } => id * 100,
        _ => 0
    };


    return s1 + a0 + a3 + res + who;
}

fn TestMatchNumbers(x) {
    match x {
        0 => "zero",
        1 => "one",
        _ => "other"
    }
}

fn TestMatchTuple(point) {
    match point {
        (0, 0) => 0,
        (x, y) => x*x + y*y
    }
}

fn TestMatchStruct(user) {
    match user {
        User { id, name } => "{id}:{name}",
        _ => "unknown"
    }
}

fn TestGuard(value) {
    match value {
        x if x > 10 => "big",
        _ => "small"
    }
}

enum Shape {
    Circle { r: f32 },
}

enum ShapeRune {
    Circle { r },
    Square { w, h }
}

fn TestMatchShapes(shape) {
    match shape {
        Circle { r } => r * 2,
        Square { w, h } => w * h,
        _ => 0
    }
}


"#;

    println!("{}", code);

    println!("→ STEP 2: Lexing\n");

    let tokens = collect_tokens(&mut Lexer::new(code));

    for (i, tok) in tokens.iter().enumerate() {
        println!(
            "  [{}] {:?}   span=({}:{})",
            i,
            tok.kind,
            tok.span.start.offset,
            tok.span.end.offset
        );
    }

    println!("\n→ STEP 3: Parsing\n");

    let mut parser = Parser::new(tokens);

let program = match parser.parse_program() {
    Ok(p) => {
        println!("✔ PARSER OK");
        p
    }
    Err(e) => {
        println!("\n❌ PARSER FAILED");
        println!("Error: {:?}", e);

        // Extract SPAN manually because ParserError has no `.span()` method
        let span_opt = match &e {
            ParserError::Message { span, .. } => Some(span),
            ParserError::Unexpected { span, .. } => Some(span),
            _ => None,
        };

        if let Some(span) = span_opt {
            println!(
                "Span: {}..{} (line {}, col {})",
                span.start.offset,
                span.end.offset,
                span.start.line,
                span.start.column
            );

            println!(
                "Source fragment: `{}`",
                &code[span.start.offset .. span.end.offset]
            );
        } else {
            println!("(No span available for this error variant)");
        }

        panic!("Parser failed, aborting test");
    }
};


    println!("\n→ STEP 4: Executing VM\n");

    use std::panic::{catch_unwind, AssertUnwindSafe};

    println!("→ STEP 4: Loading program into VM");

    let mut vm = RuneVM::new();

    let result = catch_unwind(AssertUnwindSafe(|| {
        vm.run_program(&program);
    }));

    match result {
        Ok(_) => println!("✔ VM loaded program"),
        Err(e) => {
            println!("❌ VM PANICKED DURING LOAD: {:?}", e);
            panic!("VM load failed");
        }
    }


    println!("\n→ STEP 5: Calling function\n");

    let out = match vm.call_fn("AllFeatures", &[], rune_ast::Span::dummy()) {
        Ok(v) => {
            println!("✔ Function returned value: {:?}", v);
            v
        }
        Err(e) => {
            println!("❌ CALL FAILED: {:?}", e);
            panic!("Call failed");
        }
    };

    println!("\n→ STEP 6: Normalizing output\n");

    let result = match out {
        rune_evaluator::value::Value::Number(n) => {
            println!("✔ Final result = {}", n);
            n as f64
        }
        other => {
            println!("❌ Expected number, got: {:?}", other);
            panic!("Invalid return");
        }
    };

    println!("\n============================= END TEST =================================\n");
}
