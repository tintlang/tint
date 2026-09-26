// Exercises tint_runtime::ui_session::UiSession end to end: a `state`
// declaration surviving across renders, a `click||handler` attribute
// actually reaching the render tree (see UiRenderNode::on_click) and, via
// `dispatch()`, actually toggling that state and changing what `if{}`
// renders next -- this is the real click round-trip the sandbox's
// MenuPanel-as-popover needs (previously `click||` was parsed and
// silently dropped, and there was no `state` at all -- see this
// project's compilation-status notes on "MenuPanel-as-popover").

use tint_runtime::ui_session::UiSession;

const SRC: &str = r#"
fn toggle_menu() {
    menu_open = !menu_open
}

ui fn App() {
    state menu_open = false

    Button {
        click||toggle_menu
        "Send / Request"
    }

    MenuPanel {
        if{menu_open}
        "Send money"
    }
}
"#;

const DEMO_SOURCE: &str = concat!(
    include_str!("demo_source/part1.tn"),
    include_str!("demo_source/part2.tn")
);

include!("basic.rs");
include!("demo.rs");
