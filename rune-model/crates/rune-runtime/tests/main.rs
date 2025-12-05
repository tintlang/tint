use rune_evaluator::EvalHost;

#[test]
fn test_rune_capabilities() {
    use rune_lexer::{Lexer, collect_tokens};
    use rune_parser::{Parser};
    use rune_parser::error::ParserError;
    use rune_runtime::vm::RuneVM;
    use rune_evaluator::value::Value;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    println!("\n===================== RUNE TEST START =====================\n");

let code = r#"
//     MODULE + IMPORTS

mod math {
    fn vec3(x, y, z) {
        (x, y, z)
    }

    fn add(a, b) { a + b }
}

use math::vec3;
use math::add;
use math;


//     GLOBAL DESTRUCT PATTERNS

let (x, y) = p;
let (a, (b, c)) = nested;
let (_, y2) = pair;



//     FUNCTION PARAM PATTERNS

// tuple params
fn foo1((x, y)) {
    x + y
}

// nested tuple params
fn foo2((a, (b, c))) {
    a + b + c
}

// struct params
fn foo3(User { id, name }) {
    id * 100
}

// typed param
fn typed1(x: i32) {
    x + 1
}

// typed tuple destruct
fn typed2((a: i32, b)) {
    a * 10 + b
}


//     STRUCT + ENUM DEFINITIONS

struct User {
    id{i32},
    name{string},
    age{i32},
}

enum Shape {
    Circle { r: f32 },
}

enum E2 {
    A { x },
    B { y, z },
}


//     STRUCT INIT + UPDATE + REST

fn test_updates() {
    User { ..old }
    User { ..old, age{30} }
    User { id{1}, ..other }
}


//     MAIN FEATURE TEST

fn AllFeatures() {
    let p = (10, 20, 30);
    let (x, y, z) = p;

    let s1 = x + y + z;

    let arr = [x, y, z, s1];
    let a0 = arr[0];
    let a3 = arr[3];

    let u = User { id{x}, name{"Rune"}, age{99} };

    let user2 = User {
        age{u.age + 1},
        name{"Nornse"},
        id{u.id},
    };

    // variant pattern match
    let en = E2::A { x{50} };

    let r1 = match en {
        A { x } => x * 2,
        B { y, z } => y + z,
        _ => 0
    };

    // typed pattern match
    let tval = 42;
    let r2 = match tval {
        x: i32 => x * 2,
        _ => 0,
    };

    // tuple match
    let t = (1, 2, 3);
    let r3 = match t {
        (1, b, 3) => b * 10,
        _ => 0
    };

    // struct match
    let who = match user2 {
        User { id, name } => id * 100,
        _ => 0
    };

    return s1 + a0 + a3 + r1 + r2 + r3 + who;
}


//     MISC MATCH TESTS

fn TestMatchNumbers(x) {
    match x {
        0 => "zero",
        1 => "one",
        _ => "other",
    }
}

fn TestMatchTuple(point) {
    match point {
        (0,0) => 0,
        (x,y) => x*x + y*y,
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
        _ => "small",
    }
}

fn TestMatchShapes(shape) {
    match shape {
        Circle { r } => r * 2,
        _ => 0,
    }
}


//     MAP LITERALS

fn TestMapBasic() {
    let m = map { 
        a{1}, 
        b{2}, 
        c: 3 
    };

    return m.a + m.b + m.c;
}

fn TestMapNested() {
    let m = map {
        user{ map {
            id{10},
            name{"Rune"}
        }},
        nums{ [1,2,3] }
    };

    return m.user.id + m.nums[2]; // 10 + 3
}

fn TestMapPattern() {
    let m = map { x{5}, y{9} };

    match m {
        map { x, y } => x * y,
        _ => 0
    }
}

    // ======================================================
//     GENERIC ENUM TESTS — RUNE STYLE vs RUST STYLE
// ======================================================

// ------------ Rune-style generic enums ------------
// Rune-style позволяет:
//    Some { value }
//    Err { error }
//    Ok { value }
//
// Инициализация: 
//    Option::Some { value{10} }
//    Result::Err { error{"msg"} }

enum Option<T> {
    Some { value },
    None {}
}

enum Result<T, E> {
    Ok { value },
    Err { error },
}

fn TestRuneOption() {
    let a = Option::Some { value{10} };
    let b = Option::None {};

    match a {
        Some { value } => value + 1,
        None {} => 0
    }
}

fn TestRuneResult() {
    let r1 = Result::Ok { value{"OK"} };
    let r2 = Result::Err { error{"E"} };

    match r1 {
        Ok { value } => value.len(),
        Err { error } => -1
    }
}


// ------------ Rust-style generic enums ------------
// Rust-style позволяет:
//    Some { value: expr }
//    Ok { value: expr }
//
// Инициализация:
//
//    Option2::Some { value: 123 }
//    Result2::Err { error: "fail" }

enum Option2<T> {
    Some { value: T },
    None {}
}

enum Result2<T, E> {
    Ok { value: T },
    Err { error: E },
}

fn TestRustOption() {
    let a = Option2::Some { value: 20 };
    let b = Option2::None {};

    match a {
        Some { value: v } => v * 3,
        None {} => 0
    }
}

fn TestRustResult() {
    let r = Result2::Ok { value: 5 };

    match r {
        Ok { value: x } => x * 2,
        Err { error: _ } => 0
    }
}
"#;

    // 1) Lexing
    println!("→ LEXING...");
    let tokens = collect_tokens(&mut Lexer::new(code));
    println!("✔ {} tokens", tokens.len());

    // 2) Parsing
    println!("→ PARSING...");
    let mut parser = Parser::new(tokens);

    let program = match parser.parse_program() {
        Ok(p) => {
            println!("✔ Parsing OK");
            p
        }
        Err(e) => {
            println!("\n❌ PARSER FAILED: {:?}", e);

            if let Some(span) = match &e {
                ParserError::Message { span, .. } => Some(span),
                ParserError::Unexpected { span, .. } => Some(span),
                _ => None,
            } {
                println!(
                    "At {}..{} → `{}`",
                    span.start.offset,
                    span.end.offset,
                    &code[span.start.offset .. span.end.offset]
                );
            }

            panic!("parser failed");
        }
    };

    // 3) Load into VM
    println!("→ LOADING VM...");
    let mut vm = RuneVM::new();

    let load = catch_unwind(AssertUnwindSafe(|| {
        vm.run_program(&program);
    }));

    if load.is_err() {
        panic!("❌ VM panicked during load");
    }

    println!("✔ VM ready\n");

    // 4) Run AllFeatures()
    println!("→ CALL AllFeatures()");

    let out = vm
        .call_fn("AllFeatures", &[], rune_ast::Span::dummy())
        .expect("call failed");

    let num = match out {
        Value::Number(n) => n as f64,
        other => panic!("❌ Expected number, got {:?}", other),
    };

    println!("✔ AllFeatures result = {}", num);

    // 5) Additional function tests
    println!("\n→ EXTRA TESTS");

    let t1 = vm.call_fn("TestMatchNumbers", &[Value::Number(0.0)], rune_ast::Span::dummy()).unwrap();
    let t2 = vm.call_fn("TestMatchNumbers", &[Value::Number(5.0)], rune_ast::Span::dummy()).unwrap();
    println!("TestMatchNumbers(0)  = {:?}", t1);
    println!("TestMatchNumbers(5)  = {:?}", t2);

    let tup = Value::Tuple(vec![Value::Number(3.0), Value::Number(4.0)]);
    let t3 = vm.call_fn("TestMatchTuple", &[tup], rune_ast::Span::dummy()).unwrap();
    println!("TestMatchTuple((3,4)) = {:?}", t3);

    let user_struct = Value::StructInstance {
        name: "User".into(),
        fields: vec![
            ("id".into(), Value::Number(7.0)),
            ("name".into(), Value::String("A".into())),
            ("age".into(), Value::Number(30.0)),
        ],
    };
    let t4 = vm.call_fn("TestMatchStruct", &[user_struct], rune_ast::Span::dummy()).unwrap();
    println!("TestMatchStruct(User) = {:?}", t4);

    let t5 = vm.call_fn("TestGuard", &[Value::Number(50.0)], rune_ast::Span::dummy()).unwrap();
    println!("TestGuard(50) = {:?}", t5);

    println!("\n===================== RUNE TEST END =====================\n");
}
