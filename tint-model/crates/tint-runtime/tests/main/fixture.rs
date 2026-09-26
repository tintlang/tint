// Shared fixture for the split-out `AllFeatures`/pattern-matching test
// suite (see `tests/main.rs`, which just declares `mod` for the files in
// this directory). This one giant Tint source string is the SAME text
// that used to live inline inside the single `test_tint_capabilities`
// function in `tests/main.rs` before the split -- copied verbatim, not
// re-derived, so nothing here changes what source is actually exercised.
//
// It intentionally covers roughly eighty different, mostly-unrelated
// language constructs in one blob: UI parsing, GPU kernels, borrow
// blocks, generics, control flow, pattern matching, map literals, and
// more. Most of that is PARSER/LOADER-acceptance coverage only -- a lot
// of it (GPU kernels, borrow blocks, several of the generic-fn shapes)
// isn't wired up to real runtime semantics yet, so there's nothing
// meaningful to assert about what running those functions actually
// computes. `parses_and_loads.rs` covers that half honestly, as a smoke
// test: the whole source lexes, parses, and loads into a VM without
// panicking. `pattern_matching.rs` is the other half -- the handful of
// functions in here (`AllFeatures`, `TestMatchNumbers`, `TestMatchTuple`,
// `TestMatchStruct`, `TestGuard`) that DO compute something checkable,
// upgraded from a `println!` of whatever came out to a real `assert_eq!`
// against a hand-verified expected value.

use tint_lexer::{collect_tokens, Lexer};
use tint_parser::error::ParserError;
use tint_parser::Parser;
use tint_runtime::vm::TintVM;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const SOURCE: &str = r#"
[@(main.att)]

ui fn TestXml() {
    <Column padding::20 radius::12>

        <Text text::{24, bold}>
            "Hello XML"
        </Text>

        <Button click||increment radius.top::6 padding::12>
            "Click me"
        </Button>

        <Row gap::12>
            <Icon glyph||home size::16 />
            <Icon glyph||settings size::16 />
        </Row>

    </Column>
}

ui fn TestBlock() {
    Column {
        padding::20
        radius::8

        Text {
            text::{20, medium}
            "Hello BLOCK"
        }

        Button {
            click||do_action
            padding::{left::12, right::12}
            radius.top::10
            "Open"
        }

        Row {
            gap::10
            Text { "A" }
            Text { "B" }
        }
    }
}

ui fn TestInterpolation(name) {
    Column {
        padding::20

        Text {
            text::20
            "Hello {name}"
        }

        Text {
            text::14
            "User len = {name.len()}"
        }
    }
}

ui fn TestModifiers() {
    Column {
        padding::{left::20, top::10}
        opacity::0.8
        radius::12
        background::black

        Card {
            padding::12
            radius.top::20


            Text { text::{18, bold, red}  "Animated" }
        }
    }
}

ui fn TestMultiRoot() {
    <Text text::20> "First" </Text>
    <Text text::20> "Second" </Text>
    <Text text::20> "Third" </Text>
}

ui fn TestIf(visible) {
    <Block if{visible}>
        <Text text::20> "Visible!" </Text>
    </Block>

    <Block if{!visible}>
        <Text text::20> "Hidden" </Text>
    </Block>
}

ui fn TestFor(list) {
    <Block for{item in list}>
        <Text text::16> "{item}" </Text>
    </Block>
}

ui fn TestMatch(status) {
    <Block match{status}>
        <case ready>   <Text>"OK"</Text> </case>
        <case error>   <Text>"ERR"</Text> </case>
        <case loading> <Text>"LOAD"</Text> </case>
    </Block>
}
ui fn TestClick() {
    Column {
        padding::20

        Text { text::20 "Count: {count}" }

        Button {
            click||inc
            padding::12
            text::"Add"
        }
    }
}


ui fn TestNested() {
    Column {
        padding::20

        Card {
            padding::16
            radius::12

            Column {
                gap::8

                Text { "Inner Text" }
                Row {
                    gap::6
                    Icon { glyph||user size::14 }
                    Icon { glyph||settings size::14 }
                }
            }
        }
    }
}

space Logic {
    struct Node<T> {
        id{i32},
        value{T},
        next{Option<Node<T>>},
    }

    impl Node<T> {
        fn sum(self) {
            match self.next {
                Some { value, next } => self.value + value + match next {
                    Some { value, .. } => value,
                    None {} => 0
                },
                None {} => self.value
            }
        }
    }

    struct Vec2 {
        x{f32},
        y{f32},
    }

    impl Vec2 {
        fn len(self) {
            (self.x * self.x + self.y * self.y).sqrt()
        }
    }

    fn deep_mix(a, b) {
        let n = Node {
            id{1},
            value{10},
            next{
                Option::Some {
                    value{20},
                    next{
                        Option::Some { value{30} }
                    }
                }
            }
        };

        let v = Vec2 { x{3.0}, y{4.0} };
        let s = n.sum() + v.len();

        let m = map {
            user{
                User {
                    id{ a + b },
                    name{"A"},
                    age{ s }
                }
            },
            arr{ [ (1,2) (3,4) (5, [6,7,8]) ] },
            nested{
                map {
                    x{ map { y{ map { z{99} } } } }
                }
            }
        };

        let r = match m {
            map {
                user{ User { id, age } },
                arr,
                nested{ map { x{ map { y{ map { z } } } } } }
            } => {
                id + age + arr[2][1] + z
            },
            _ => 0
        };

        r + s
    }
}

fn AdvancedMonster(x, y {10}) {
    let a = (1, (2, (3, (4, [x, y, x+y]))));

    let b = match a {
        (x, (y, (z, (k, arr)))) => {
            let f = |u| u * 2;
            f(x + y + z + k + arr[2])
        }
    };

    let c = borrow(x) {
        borrow@group(y) {
            x + y + b
        }
    };

    let d = Logic::deep_mix(x, y);

    (b + c + d) * 2
}

fn MonsterTestMaster() {
    AdvancedMonster(5)
}

impl Texture {
    fn reset() { 0 }
}

impl Box<T> {
    fn unwrap(self) { self.value }
}

fn TestImplCall(tex: Texture) {
    tex.downscale();
}

space LogicTest {
    impl Texture {
        fn apply(self) { 1 }
    }

    fn run(t: Texture) {
        t.apply()
    }
}

space G1 {
    kernel blur(img: Texture) -> Texture {
        img
    }
}

space L1 {
    fn add(a, b) { a + b }
    struct User { id{i32} }
}

kernel invert(tex: Texture) -> Texture {
    let id = global_id();
    let px = tex.read(id);
    write(px);
}

fn TestGpuCall(frame: Texture) -> Texture {
    let out = invert.gpu(frame);
    out
}

fn TestGpuChain(frame: Texture) -> Texture {
    let a = preprocess(frame).gpu(frame);
    let b = invert.gpu(a);
    b
}

fn TestGpuNested(frame: Texture) -> Texture {
    invert.gpu( preprocess(frame).gpu(frame) )
}

fn TestGpuBinary(frame: Texture) -> Texture {
    let x = invert.gpu(frame);
    x
}


fn TestKernelBasic(frame: Texture) {
    let out = invert.gpu(frame);
    out // return texture reference
}

fn TestGpuReturn(frame: Texture) -> Texture {
    return invert.gpu(frame);
}

kernel brightness(tex: Texture, amount: f32) -> Texture {
    let id = global_id();
    let px = tex.read(id);
    write(px * amount);
}

fn TestKernelParams(frame: Texture) {
    let out = brightness.gpu(frame, 2.0);
    out
}

fn MegaNested() {
    let a = (
        1,
        (2, (3, (4, 5))),
        User {
            id{10},
            name{"Tint"},
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
        borrow@group(invert) {
            // nested borrow blocks allowed
            convolve(frame, invert)
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
    let b = id("Tint")

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
    let b = wrap("Tint")
    let c = wrap(true)

    (a == 10) && (b == "Tint") && c
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
    borrow(frame) {         
        sobel(frame)
    }

    borrow@group(invert) {  
        sort(invert)
    }
}

mod engine {
    fn blur() {
        borrow(img) { sobel(img) }
    }
}

fn [@(strict)] gpu() {
    borrow(frame) {   // strict
        convolve(frame, invert)
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
    let u = make_user { name{"Tint"}, age{20} };
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
    let s = "Tint";
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

    let u = User { id{x}, name{"Tint"}, age{99} };

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
            name{"Tint"}
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

//     GENERIC ENUM TESTS — TINT STYLE vs RUST STYLE

// ------------ Tint-style generic enums ------------
// Tint-style allows:
//    Some { value }
//    Err { error }
//    Ok { value }
//
// Initialization:
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

fn TestTintOption() {
    let a = Option::Some { value{10} };
    let b = Option::None {};

    match a {
        Some { value } => value + 1,
        None {} => 0
    }
}

fn TestTintResult() {
    let r1 = Result::Ok { value{"OK"} };
    let r2 = Result::Err { error{"E"} };

    match r1 {
        Ok { value } => value.len(),
        Err { error } => -1
    }
}


// ------------ Rust-style generic enums ------------
// Rust-style allows:
//    Some { value: expr }
//    Ok { value: expr }
//
// Initialization:
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

/// Lexes, parses, and loads `SOURCE` into a fresh `TintVM`, panicking with
/// the same diagnostics the original single giant test printed (source
/// span for a parser error; a plain message if the VM panics while
/// loading). Every split-out test file calls this once to get its own VM
/// -- `TintVM` isn't `Clone`, and there's no cross-test state to share by
/// reusing one instance, so re-loading per test (cheap: this is a small
/// in-memory source string, not I/O) is simpler than threading a shared
/// instance through `OnceLock`/`Mutex` for no real benefit.
pub fn load_vm() -> TintVM {
    let tokens = collect_tokens(&mut Lexer::new(SOURCE));

    let mut parser = Parser::new(tokens);

    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            if let Some(span) = match &e {
                ParserError::Message { span, .. } => Some(span),
                ParserError::Unexpected { span, .. } => Some(span),
            } {
                panic!(
                    "parser failed at {}..{} -> `{}`: {:?}",
                    span.start.offset,
                    span.end.offset,
                    &SOURCE[span.start.offset..span.end.offset],
                    e
                );
            }
            panic!("parser failed: {:?}", e);
        }
    };

    let mut vm = TintVM::new();

    let load = catch_unwind(AssertUnwindSafe(|| {
        vm.run_program(&program);
    }));

    if load.is_err() {
        panic!("VM panicked while loading the shared fixture source");
    }

    vm
}
