// Components with props and their own state (tint-parser/src/components.rs).

fn texts(nodes: &[std::rc::Rc<tint_runtime::ui::render::UiRenderNode>], out: &mut Vec<String>) {
    for n in nodes {
        if let Some(t) = &n.text {
            out.push(t.clone());
        }
        texts(&n.children, out);
    }
}

fn all_text(session: &mut UiSession) -> Vec<String> {
    let tree = session.render().expect("render failed");
    let mut out = Vec::new();
    texts(&tree, &mut out);
    out
}

const COUNTERS: &str = r#"
ui fn App() {
    component Counter(label: string, start: i32 {0}) {
        state n = start
        fn inc() { n = n + 1 }
        fn twice() { inc() inc() }
        Button { click||inc "{label}: {n}" }
        Wide { click||twice "x2" }
    }
    Counter { label::"A" start::5 }
    Counter { label::"B" }
}
"#;

#[test]
fn every_use_has_its_own_props_and_state() {
    let mut s = UiSession::new(COUNTERS, "App").expect("session");
    let tree = s.render().expect("render");
    let handlers: Vec<_> = tree.iter().flat_map(|c| c.children.iter()).filter_map(|n| n.on_click.clone()).collect();
    assert_eq!(handlers, ["Counter__1__inc", "Counter__1__twice", "Counter__2__inc", "Counter__2__twice"]);
    assert_eq!(all_text(&mut s), ["A: 5", "x2", "B: 0", "x2"]);
    s.dispatch("Counter__1__inc").expect("dispatch");
    assert_eq!(all_text(&mut s), ["A: 6", "x2", "B: 0", "x2"]);
    // a component fn calling another one of the same use
    s.dispatch("Counter__2__twice").expect("dispatch");
    assert_eq!(all_text(&mut s), ["A: 6", "x2", "B: 2", "x2"]);
}

#[test]
fn slots_variants_and_nested_components() {
    let src = r#"
ui fn App() {
    state who = "me"
    component Badge(text: string) {
        padding::4
        Label { "{text}" }
    }
    component Card(title: string) {
        state open = false
        fn toggle() { open = !open }
        variant::wide { margin::2 }
        Head { click||toggle "{title}" }
        Badge { text::"new" }
        slot::content
        Foot { if{open} "open" }
    }
    Card { title::{ who } variant::wide
        Body { "hello {who}" }
    }
}
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    let tree = s.render().expect("render");
    assert_eq!(tree[0].tag, "Card");
    assert!(tree[0].style.iter().any(|(k, v)| k == "margin" && v == "2px"), "{:?}", tree[0].style);
    let tags: Vec<_> = tree[0].children.iter().map(|c| c.tag.clone()).collect();
    assert_eq!(tags, ["Head", "Badge", "Body"]);
    assert_eq!(all_text(&mut s), ["me", "new", "hello me"]);
    s.dispatch("Card__1__toggle").expect("dispatch");
    assert_eq!(all_text(&mut s), ["me", "new", "hello me", "open"]);
}

#[test]
fn props_follow_the_state_they_are_bound_to_and_shadowing_is_respected() {
    let src = r#"
fn bump() { count = count + 1 }
ui fn App() {
    state count = 1
    component Show(value: i32) {
        Row { for{ value in [10, 20] } "{value}" }
        Total { "{value}" }
    }
    Click { click||bump "+" }
    Show { value::{ count } }
}
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    // the loop's own `value` is not the prop
    assert_eq!(all_text(&mut s), ["+", "10", "20", "1"]);
    s.dispatch("bump").expect("dispatch");
    assert_eq!(all_text(&mut s), ["+", "10", "20", "2"]);
}

fn parse_error(src: &str) -> String {
    match UiSession::new(src, "App") {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

#[test]
fn component_use_errors_are_reported() {
    let missing = parse_error("ui fn App() { component C(a: string) { Box { \"{a}\" } } C { } }");
    assert!(missing.contains("needs the prop `a`"), "{missing}");
    let looped = parse_error(
        "ui fn App() { state xs = [1, 2] component C() { state n = 0 fn f() { n = n + 1 } Box { click||f \"{n}\" } } Row { for{ x in xs } C { } } }",
    );
    assert!(looped.contains("cannot be used inside a `for`"), "{looped}");
}

#[test]
fn props_only_components_work_inside_a_loop() {
    let src = r##"
ui fn App() {
    state xs = [1, 2, 3]
    component Item(n: i32) { Cell { "#{n}" } }
    Row { for{ x in xs } Item { n::{ x } } }
}
"##;
    let mut s = UiSession::new(src, "App").expect("session");
    assert_eq!(all_text(&mut s), ["#1", "#2", "#3"]);
}

#[test]
fn resource_and_lifecycle_attributes_reach_the_node() {
    let src = r#"
ui fn App() {
    component Feed(url: string) {
        resource items = fetch_text(url)
        state seen = 0
        fn tick() { seen = seen + 1 }
        effect||tick deps||{url}
        role||"feed" aria_label||"Items"
        Box { "{items} {items_loading}" }
    }
    Feed { url::"/a.txt" }
}
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    let tree = s.render().expect("render");
    let root = &tree[0];
    let attr = |n: &str| root.attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    assert_eq!(attr("data-tint-mount").as_deref(), Some("Feed__1__items_load"));
    assert_eq!(attr("data-tint-effect").as_deref(), Some("Feed__1__tick"));
    assert_eq!(attr("data-tint-deps").as_deref(), Some("/a.txt"));
    assert_eq!(attr("role").as_deref(), Some("feed"));
    assert_eq!(attr("aria-label").as_deref(), Some("Items"));
}

#[test]
fn aria_booleans_are_written_as_text() {
    let src = r#"
ui fn App() {
    state on = true
    Box { aria_pressed||{on} aria_hidden||false disabled||{on} "x" }
}
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    let tree = s.render().expect("render");
    let attr = |n: &str| tree[0].attrs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    assert_eq!(attr("aria-pressed").as_deref(), Some("true"));
    assert_eq!(attr("aria-hidden").as_deref(), Some("false"));
    // A plain HTML boolean is present (empty) when true.
    assert_eq!(attr("disabled").as_deref(), Some(""));
}

#[test]
fn derived_names_stand_for_expressions() {
    let src = r#"
ui fn App() {
    state price = 4
    state qty = 3
    derived total = price * qty
    derived label = "total {total}"
    Box { "{label}" }
    component Line(n: i32) {
        state k = 2
        derived twice = n * k
        Row { "{twice}" }
    }
    Line { n::5 }
}
fn more() { qty = qty + 1 }
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    assert_eq!(all_text(&mut s), ["total 12", "10"]);
    s.dispatch("more").expect("dispatch");
    assert_eq!(all_text(&mut s), ["total 16", "10"]);
}

#[test]
fn persist_state_reads_and_writes_storage() {
    let src = r#"
ui fn App() {
    persist state name = "anon"
    Box { "{name}" }
}
fn rename() { name = "kim" }
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    assert_eq!(all_text(&mut s), ["anon"]);
    s.dispatch("rename").expect("dispatch");
    assert_eq!(all_text(&mut s), ["kim"]);
}

#[test]
fn form_validates_touched_and_submits() {
    let src = r#"
ui fn App() {
    component Signup() {
        state sent = 0
        form f send {
            field email = "" "required, email"
            field pw = "" "min:4"
        }
        fn send() { sent = sent + 1 }
        Box { "{f_email_error}|{f_email_msg}|{f_valid}|{sent}" }
        Button { click||f_submit "go" }
        Button { click||f_email_blur "blur" }
    }
    Signup {}
}
"#;
    let mut s = UiSession::new(src, "App").expect("session");
    let first = all_text(&mut s);
    assert_eq!(first[0], "Required||false|0");
    let clicks = s.dispatch("Signup__1__f_email_blur").expect("blur");
    drop(clicks);
    assert_eq!(all_text(&mut s)[0], "Required|Required|false|0");
}
