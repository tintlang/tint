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

[@(main.att)]

fn MegaNested() {
    let a = (
        1,
        (2, (3, (4, 5))),
        User {
            id{10},
            name{"Rune"},
            age{
                (1 + 2) * 3
            }
        }
    );

    let b = map {
        x{ map {
            y{ map {
                z{ 99 }
            }}
        }},
        list{ [ (1,2) (3,4) (5, [6,7,8]) ] },
        user{
            User {
                id{ a.2.id },
                name{"Deep"},
                age{ a.2.age + 1 }
            }
        }
    };

    let c = match a.1 {
        (x, (y, (z, k))) => x + y + z + k,
        _ => 0,
    };

    let d = match b {
        map {
            x{ map {
                y{ map { z } }
            }},
            user{ User { id, name } },
            list
        } => z + id + name.len(),
        _ => 0
    };

    let op = Option::Some { value{ map {
        inner{ [1,2,3] },
        deep{ User { id{5}, name{"X"}, age{7} } }
    }}};

    let r = match op {
        Some { value{ inner, deep } } => {
            let mul = |a, b| a * b;
            mul(inner[1], deep.age)
        },
        None {} => 0
    };

    borrow(frame) {
        borrow@group(kernel) {
            // nested borrow blocks allowed
            convolve(frame, kernel)
        }
    }

    // combine results for sanity
    c + d + r
}

struct Box<T> {
    value{T}
}

fn TestStructGeneric() {
    let b = Box { value{50} }
    b.value + 1
}

enum Option<T> {
    Some { value },
    None {}
}

fn TestOptionGeneric() {
    let a = Option::Some { value{10} }
    let b = Option::None {}

    match a {
        Some { value } => value + 1,
        None {} => 0
    }
}

struct Vec3<T> {
    x{T}, y{T}, z{T}
}

fn length<T>(v: Vec3<T>) { 0 } // fake test, just checks parsing

fn id<T>(x: T) -> T { x }

fn TestReturnGeneric() {
    let a = id(99)
    let b = id("Rune")

    (a + 1) == 100 && b.len() == 4
}

    enum Result<T, E> {
    Ok { value },
    Err { error },
}

fn TestGenericResult() {
    let r = Result::Ok { value{50} }

    match r {
        Ok { value } => value * 2,
        Err { error } => 0
    }
}


fn Pair<K, V>(a: K, b: V) {
    (a, b)
}

fn TestPair() {
    let p = Pair(1, "A")
    p.0 + p.1.len()   // 1 + 1 = 2
}

fn pick<T>(x: T, y: T {x}) { y }

fn TestGenericDefault() {
    pick(5, 10) + pick(5)  // 10 + 5
}


fn wrap<T>(x: T) { x }

fn TestWrap() {
    let a = wrap(10)
    let b = wrap("Rune")
    let c = wrap(true)

    (a == 10) && (b == "Rune") && c
}


struct Sprite {
    @kit.position2d
    @kit.size
    @kit.uv
    @kit.color
}

struct Sprite{
    @kit.transform2d {
        @kit.position {
            x{f32},
            y{f32},
        }

        @kit.rotation {
            angle{f32},
        }
    }
}


fn [@(strict)] gpu() {
    let r = borrow(frame)
}

fn gpu() {
    let r = borrow @strict (frame)
}

fn [@(strict)] f() {
    borrow(x)       
    borrow@group(x)  
    borrow@strict(x) 
}

fn normal() {
    borrow(x)         
    borrow@strict(x)  
    borrow@group(x)   
}

fn [@(strict)] gpu_process() {
    borrow(frame) {          // становится core borrow
        sobel(frame)
    }

    borrow@group(kernel) {   // используется group borrow
        sort(kernel)
    }
}

mod engine {
    fn blur() {
        borrow(img) { sobel(img) } // strict по умолчанию
    }
}

fn [@(strict)] gpu() {
    borrow(frame) {   // strict
        convolve(frame, kernel)
        normalize(frame)
    }

    borrow@group(samples) {   // high-level
        samples.push(42)
    }
}

fn [@(speed, speed)] TestFor1(n) {
    let mut sum = 0;

    for i in 0..n {
        sum = sum + i;
    }

    sum
}
    
fn TestIf1(x) {
    if (x > 10) {
        1
    } else {
        0
    }
}

fn TestIf2(x) {
    let mut a = 0;
    if (x == 5) {
        a = 10;
    }
    a
}

fn TestIfNested(x, y) {
    if (x > y) {
        if (x > 100) { 1 } else { 2 }
    } else {
        3
    }
}

fn TestWhile1(n) {
    let mut sum = 0;
    let mut i = 0;

    while (i < n) {
        sum = sum + i;
        i = i + 1;
    }

    sum
}

fn TestWhileBreak() {
    let mut i = 0;

    while (true) {
        if (i == 3) { break; }
        i = i + 1;
    }

    i
}

fn TestForBreak() {
    let mut res = 0;

    for i in 0..10 {
        if (i == 4) { break; }
        res = res + i;
    }

    res
}

export fn exported_fn(a) { a + 1 }
export struct Ex { id{i32} }

type Number = union(i32 | f32 | f64)

fn TestUnionParsing(x: Number) {
    1  // dummy
}

fn TestTupleDefault1((a, b, c) {10}) {
    a + b + c     // 10 + 10 + 10 = 30
}

fn TestTupleDefault2((x, (y, z)) {5}) {
    x * y + z     // 5 * 5 + 5 = 30
}

fn inner((p1, p2) {3}) {
    (p1, p2)
}

fn TestTupleDefaultMatch2() {
    let tmp = (x, y);

    match tmp {
        (3, 3) => 1
        _ => 0
    }
}

struct User {
    id{i32},
    name{string},
}

fn TestStructDefault1(User { id, name } {"X"}) {
    id.len() + name.len()  // "X".len = 1 → 1 + 1 = 2
}

fn TestStructDefault2(User { id, name } { 99 }) {
    id + name.len()        // id=99, name=99?? (string?)  
}

struct Pair {
    a{i32},
    b{i32},
}

fn TestStructDefault3(Pair { a, b } {7}) {
    a + b   // 14
}


fn TestStructUpdateField() {
    let u = User { id{1}, name{"A"}, age{10} };
    let u2 = User { age{u.age + 1}, ..u };
    u2.age
}

fn TestDefaultParams(a, b{10}, c: i32{5}) {
    a + b + c
}

fn TestDefaultParams2(x{1}) { x * 2 }

fn make_user(name, age) {
    User { name{name}, age{age}, id{0} }
}

fn TestNamedCalls() {
    let u = make_user { name{"Rune"}, age{20} };
    u.age + 1
}

fn TestTypedArrays() {
    let xs: List<i32> = [1,2,3];
    xs[0] + xs[2]
}

fn TestLambdas() {
    let add1 = |x| x + 1;
    let mul = |a, b| a * b;

    let r1 = add1(9);
    let r2 = mul(2, 3);

    r1 + r2 // 10 + 6 = 16
}

    fn MakePair() {
    (10, 20)
}

fn TestTupleReturn() {
    let (a, b) = MakePair();
    a * b
}

fn TestArrayAssign() {
    let mut arr = [1,2,3];
    arr[1] = 10;
    arr[1] + arr[2]
}

fn TestMapAssign() {
    let mut m = map { a{1}, b{2} };
    m.b = 5;
    m.b * 2
}

fn TestMutParam(mut x) {
    x = x + 10;
    x
}

fn TestMoveSemantics() {
    let s = "Rune";
    let t = s;
    t.len()   // s is moved, but runtime ignore for now
}

fn TestMutLocal() {
    let mut x = 10;
    x = x + 1;
    x
}


fn TestStructFieldAssign() {
    let mut u = User { id{1}, name{"M"}, age{10} };
    u.age = 20;
    u.age
}


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

fn __global_tests__() {
    let (x, y) = p;
    let (a, (b, c)) = nested;
    let (_, y2) = pair;
}

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
        c{3}, 
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

//     GENERIC ENUM TESTS — RUNE STYLE vs RUST STYLE

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
