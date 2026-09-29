//! Subtree reuse must never change what a render produces, only how much
//! work it takes.

use std::rc::Rc;

use tint_runtime::ui::render::UiRenderNode;
use tint_runtime::ui_session::UiSession;

const SOURCE: &str = r#"
fn inc() { count = count + 1 }
fn toggle() { show = !show }
fn add_item() { items.push(items.len() + 1) }
fn set_light() { theme = "light" }
fn set_dark() { theme = "dark" }
fn label(n) { "n=" + n }
fn bump_impure() { impure = impure + 1 }

ui fn App() {
    state count = 0
    state show = true
    state items = [1, 2, 3]
    state impure = 0
    theme::dark {
        tokens { accent::#111111 }
        Page {
            Static { padding::4 Text { "always the same" } }
            Counter { Text { "count {count}" } }
            Row { for{x in items} Cell { key::"c{x}" Text { "item {x}" } } }
            Maybe { if{show} Text { "shown {count}" } }
            Called { Text { "{label(count)}" } }
            Nested { for{x in items} Inner { for{y in items} Pair { Text { "{x}-{y}-{count}" } } } }
        }
    }
    theme::light {
        Page { Text { "light {count}" } }
    }
}
"#;

fn strip(nodes: &[Rc<UiRenderNode>]) -> Vec<UiRenderNode> {
    nodes.iter().map(|n| (**n).clone()).collect()
}

fn session(reuse: bool) -> UiSession {
    let mut session = UiSession::new(SOURCE, "App").expect("session");
    session.set_reuse(reuse);
    session
}

#[test]
fn reused_renders_are_identical_to_fresh_renders_through_many_events() {
    let mut fast = session(true);
    let mut fresh = session(false);
    assert_eq!(
        strip(&fast.render().unwrap()),
        strip(&fresh.render().unwrap())
    );

    let events = [
        "inc",
        "inc",
        "toggle",
        "add_item",
        "inc",
        "toggle",
        "toggle",
        "set_light",
        "inc",
        "add_item",
        "set_dark",
        "inc",
        "add_item",
        "bump_impure",
        "toggle",
    ];
    for event in events {
        let a = fast.dispatch(event).unwrap();
        let b = fresh.dispatch(event).unwrap();
        assert_eq!(strip(&a), strip(&b), "diverged after `{event}`");
    }
}

fn find<'a>(nodes: &'a [Rc<UiRenderNode>], tag: &str) -> Option<&'a Rc<UiRenderNode>> {
    for node in nodes {
        if node.tag == tag {
            return Some(node);
        }
        if let Some(found) = find(&node.children, tag) {
            return Some(found);
        }
    }
    None
}

fn texts(node: &UiRenderNode, out: &mut Vec<String>) {
    if let Some(text) = &node.text {
        out.push(text.clone());
    }
    for child in &node.children {
        texts(child, out);
    }
}

fn all_text(node: &UiRenderNode) -> Vec<String> {
    let mut out = Vec::new();
    texts(node, &mut out);
    out
}

#[test]
fn an_unchanged_subtree_is_the_same_allocation_and_a_changed_one_is_not() {
    let mut session = session(true);
    let first = session.render().unwrap();
    let second = session.dispatch("inc").unwrap();

    // `Static` reads nothing, so it is reused as is.
    assert!(Rc::ptr_eq(
        find(&first, "Static").unwrap(),
        find(&second, "Static").unwrap()
    ));
    // `Counter` reads `count`, which changed.
    assert!(!Rc::ptr_eq(
        find(&first, "Counter").unwrap(),
        find(&second, "Counter").unwrap()
    ));
    assert_eq!(all_text(find(&second, "Counter").unwrap()), ["count 1"]);
    // `Row` only reads `items`.
    assert!(Rc::ptr_eq(
        find(&first, "Row").unwrap(),
        find(&second, "Row").unwrap()
    ));
}

#[test]
fn a_subtree_with_a_call_is_never_reused_because_it_may_have_side_effects() {
    let mut session = session(true);
    let first = session.render().unwrap();
    let second = session.dispatch("bump_impure").unwrap();
    assert!(!Rc::ptr_eq(
        find(&first, "Called").unwrap(),
        find(&second, "Called").unwrap()
    ));
}

#[test]
fn a_change_to_a_looped_variable_only_rebuilds_what_read_it() {
    let mut session = session(true);
    session.render().unwrap();
    let after_add = session.dispatch("add_item").unwrap();
    let row = find(&after_add, "Row").unwrap();
    assert_eq!(row.children.len(), 4);
    assert_eq!(all_text(&row.children[3]), ["item 4"]);
}
