use std::collections::HashMap;

use tint::{FromTint, IntoTint, Tint, Value};

#[derive(Debug, PartialEq, IntoTint, FromTint)]
struct Point {
    x: f64,
    y: f64,
}

#[tint::export]
fn area(w: f64, h: f64) -> f64 {
    w * h
}

#[tint::export(name = "len_of")]
fn length(p: Point) -> f64 {
    (p.x * p.x + p.y * p.y).sqrt()
}

#[tint::export]
fn parse_num(text: String) -> Result<f64, String> {
    text.trim().parse::<f64>().map_err(|e| e.to_string())
}

#[tint::export]
fn shifted(p: Point, by: f64) -> Point {
    Point { x: p.x + by, y: p.y + by }
}

#[tint::export]
fn total(items: Vec<f64>) -> f64 {
    items.iter().sum()
}

#[tint::export]
fn first_word(text: String) -> Option<String> {
    text.split_whitespace().next().map(str::to_string)
}

const SOURCE: &str = r#"
struct Point { x: number, y: number }

fn rect() { area(3, 4) }
fn hyp() { len_of(Point { x: 3, y: 4 }) }
fn moved() { shifted(Point { x: 1, y: 2 }, 10) }
fn sum3() { total([1, 2, 3]) }
fn parsed() { parse_num(" 42 ") }
fn bad() { parse_num("nope") }
fn double(n) { n * 2 }
"#;

#[test]
fn rust_calls_tint_with_typed_values() {
    let mut tint = Tint::new(SOURCE).unwrap();
    assert_eq!(tint.call::<f64>("double", (21.0,)).unwrap(), 42.0);
    assert_eq!(tint.call::<i32>("double", (4,)).unwrap(), 8);
}

#[test]
fn tint_calls_exported_rust() {
    let mut tint = Tint::new(SOURCE).unwrap();
    assert_eq!(tint.call::<f64>("rect", ()).unwrap(), 12.0);
    assert_eq!(tint.call::<f64>("hyp", ()).unwrap(), 5.0);
    assert_eq!(tint.call::<f64>("sum3", ()).unwrap(), 6.0);
    assert_eq!(tint.call::<Result<f64, String>>("parsed", ()).unwrap(), Ok(42.0));
    assert_eq!(tint.call::<Point>("moved", ()).unwrap(), Point { x: 11.0, y: 12.0 });
}

#[test]
fn result_becomes_tint_result() {
    let mut tint = Tint::new(SOURCE).unwrap();
    let ok = tint.call::<Result<f64, String>>("parsed", ()).unwrap();
    assert_eq!(ok, Ok(42.0));
    let err = tint.call::<Result<f64, String>>("bad", ()).unwrap();
    assert!(err.unwrap_err().contains("invalid float"));
}

#[test]
fn explicit_natives_macro_matches_registration() {
    let natives = tint::natives![area, length];
    let names: Vec<_> = natives.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["area", "len_of"]);
    let mut tint = Tint::with_natives("fn r() { area(2, 5) }", &natives).unwrap();
    assert_eq!(tint.call::<f64>("r", ()).unwrap(), 10.0);
}

#[test]
fn argument_count_and_type_are_checked() {
    let mut tint = Tint::new("fn a() { area(1) }\nfn b() { area(\"x\", 2) }").unwrap();
    assert!(tint.call::<f64>("a", ()).unwrap_err().to_string().contains("expected 2"));
    assert!(tint.call::<f64>("b", ()).unwrap_err().to_string().contains("argument 1"));
}

#[test]
fn conversions_round_trip() {
    assert_eq!(first_word("hello world".into()), Some("hello".to_string()));
    let v = Some(3.0_f64).into_tint();
    assert_eq!(Option::<f64>::from_tint(&v).unwrap(), Some(3.0));
    assert_eq!(Option::<f64>::from_tint(&Value::Unit).unwrap(), None);
    let mut map = HashMap::new();
    map.insert("a".to_string(), vec![1.0, 2.0]);
    let back = HashMap::<String, Vec<f64>>::from_tint(&map.clone().into_tint()).unwrap();
    assert_eq!(back, map);
    assert!(u8::from_tint(&Value::Number(300.0)).is_err());
    assert!(i32::from_tint(&Value::Number(1.5)).is_err());
}

#[test]
fn ui_session_sees_exports() {
    let source = r#"ui fn App() { state n = area(2, 3) Column { Text { "{n}" } } }"#;
    let mut session = Tint::ui(source, "App").unwrap();
    let tree = session.render().unwrap();
    assert!(format!("{tree:?}").contains('6'));
}

#[tint::export]
async fn slow_double(n: f64) -> Result<f64, String> {
    if n < 0.0 { Err("negative".into()) } else { Ok(n * 2.0) }
}

#[tint::export]
fn each_twice(items: Vec<f64>, f: tint::Callback) -> Result<f64, String> {
    let mut total = 0.0;
    for item in items {
        total += f.call::<f64>((item,))?;
    }
    Ok(total)
}

#[test]
fn async_export_returns_result_without_callback() {
    let (_, native) = slow_double::native();
    let out = native(&[tint::Value::Number(4.0)]).unwrap();
    assert_eq!(format!("{out}"), format!("{}", tint::IntoTint::into_tint(Ok::<f64, String>(8.0))));
    let (_, native) = slow_double::native();
    let err = native(&[tint::Value::Number(-1.0)]).unwrap();
    assert!(format!("{err}").contains("negative"));
}

#[test]
fn async_export_delivers_to_callback() {
    let (_, native) = slow_double::native();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let sink = seen.clone();
    let cb = tint::Callback::new(move |args| { *sink.borrow_mut() = format!("{}", args[0]); Ok(tint::Value::Unit) });
    let out = native(&[tint::Value::Number(5.0), tint::IntoTint::into_tint(cb)]).unwrap();
    assert!(matches!(out, tint::Value::Unit));
    assert!(seen.borrow().contains("10"));
}
