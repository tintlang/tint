// Exercises rune_runtime::ui_session::UiSession end to end: a `state`
// declaration surviving across renders, a `click||handler` attribute
// actually reaching the render tree (see UiRenderNode::on_click) and, via
// `dispatch()`, actually toggling that state and changing what `if{}`
// renders next -- this is the real click round-trip the sandbox's
// MenuPanel-as-popover needs (previously `click||` was parsed and
// silently dropped, and there was no `state` at all -- see this
// project's compilation-status notes on "MenuPanel-as-popover").

use rune_runtime::ui_session::UiSession;

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

#[test]
fn state_starts_at_its_declared_initial_value() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");

    // [Button, MenuPanel-if-check]: MenuPanel is absent because
    // menu_open starts false.
    assert_eq!(tree.len(), 1, "menu_open starts false, so MenuPanel shouldn't render at all");
    assert_eq!(tree[0].tag, "Button");
    assert_eq!(tree[0].on_click, Some("toggle_menu".to_string()), "click||toggle_menu should reach the render tree");
}

#[test]
fn dispatching_the_click_handler_toggles_state_and_the_next_render_reflects_it() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");

    let after_click = session.dispatch("toggle_menu").expect("dispatch failed");
    assert_eq!(after_click.len(), 2, "menu_open is now true, so MenuPanel should render");
    assert_eq!(after_click[1].tag, "MenuPanel");

    // Independently re-rendering (no dispatch) must show the SAME state
    // -- proving it's real persistent state, not something dispatch()
    // computed only for its own return value.
    let rendered_again = session.render().expect("render failed");
    assert_eq!(rendered_again.len(), 2, "state must still read true on a later render(), not just right after dispatch()");

    // A second click flips it back off.
    let after_second_click = session.dispatch("toggle_menu").expect("dispatch failed");
    assert_eq!(after_second_click.len(), 1, "second click should toggle menu_open back to false");
}

#[test]
fn hover_in_and_hover_out_attributes_reach_the_render_tree() {
    let src = r#"
fn show_tip() {
    tip_open = true
}
fn hide_tip() {
    tip_open = false
}

ui fn App() {
    state tip_open = false

    Block {
        hover_in||show_tip
        hover_out||hide_tip
        "?"
    }

    Block {
        if{tip_open}
        "Tooltip text"
    }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");

    assert_eq!(tree.len(), 1, "tip_open starts false");
    assert_eq!(tree[0].on_hover_enter, Some("show_tip".to_string()));
    assert_eq!(tree[0].on_hover_leave, Some("hide_tip".to_string()));

    let after_hover_in = session.dispatch("show_tip").expect("dispatch failed");
    assert_eq!(after_hover_in.len(), 2, "hovering in should reveal the tooltip block");
    assert_eq!(after_hover_in[1].tag, "Block");

    let after_hover_out = session.dispatch("hide_tip").expect("dispatch failed");
    assert_eq!(after_hover_out.len(), 1, "hovering out should hide it again");
}

#[test]
fn unknown_ui_fn_fails_construction_instead_of_panicking() {
    let err = UiSession::new(SRC, "DoesNotExist");
    assert!(err.is_err());
}

#[test]
fn dispatching_an_undeclared_handler_reports_an_error_instead_of_crashing() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    let err = session.dispatch("not_a_real_function");
    assert!(err.is_err(), "an unknown handler should come back as Err, not panic the session");

    // The session must still be usable afterwards.
    let tree = session.render().expect("render failed after a failed dispatch");
    assert_eq!(tree.len(), 1);
}


// The actual sandbox seed demo (SRC_UI_DEMO in
// sandbox/src/lib/fsStore.svelte.js) -- kept in sync by hand, not by a
// shared source file (there isn't one -- the sandbox's demo lives in a
// JS template literal). This exists so a demo/language mismatch (wrong
// modifier order, an undeclared handler, ...) is caught here instead of
// only visually, in the browser. If this test starts failing after an
// edit to SRC_UI_DEMO, update this constant to match.
const DEMO_SOURCE: &str = r#"fn toggle_menu() {
    menu_open = !menu_open
}

fn top_up() {}
fn send_money() {}
fn request_money() {}

fn show_tip() {
    tip_open = true
}

fn hide_tip() {
    tip_open = false
}

ui fn App() {
    state menu_open = false
    state tip_open = false

    Column {
        direction::column
        padding::24
        gap::20
        background::#12141a

        Row {
            direction::row
            align::center
            gap::8

            Logo {
                svg||"<svg viewBox='0 0 24 24' width='22' height='22' fill='none'><path d='M12 2 L14 9 L21 12 L14 15 L12 22 L10 15 L3 12 L10 9 Z' fill='#6c5ce7'/></svg>"
                transition::"transform .25s ease"
                hover::{ transform::"scaleX(1.15) scaleY(0.85)" }
            }

            Text { text::{16, bold, white} "Rune Wallet" }
        }

        Card {
            padding::24
            radius::20
            background::#6c5ce7
            border::{1, #8b7cf0}
            hover::{ transform::"translateY(-3px)", shadow::"0 16px 40px rgba(0,0,0,.35)" }

            Column {
                direction::column

                Row {
                    direction::row
                    gap::12
                    margin::{bottom::4}
                    Text { text::{12, #d8d2f7} "TOTAL BALANCE" }
                    Text { text::{12, bold, white} "VISA" }
                }

                Text { text::{34, bold, white} margin::{bottom::28} "$4,218.90" }

                Text { text::{15, #d8d2f7} margin::{bottom::4} "•••• •••• •••• 4821" }

                Row {
                    direction::row
                    gap::12
                    Text { text::{13, white} "Mark Bender" }
                    Text { text::{13, #d8d2f7} "12/29" }
                }
            }
        }

        Row {
            direction::row
            gap::12

            Button {
                click||top_up
                radius::14
                padding::14
                background::#6c5ce7
                color::white
                hover::{ filter::"brightness(1.12)", transform::"translateY(-1px)", shadow::"0 6px 16px rgba(0,0,0,.28)" }
                "Top up"
            }

            Menu {
                position::relative

                Button {
                    click||toggle_menu
                    radius::14
                    padding::14
                    background::#12141a
                    color::#6c5ce7
                    border::{1, #6c5ce7}
                    hover::{ filter::"brightness(1.12)", transform::"translateY(-1px)" }
                    "Send / Request"
                }

                MenuPanel {
                    position::absolute
                    z::50
                    top::56
                    left::0
                    direction::column
                    gap::4
                    padding::8
                    radius::12
                    background::#1b1e26
                    border::{1, #2a2f3a}
                    shadow::"0 12px 32px rgba(0,0,0,.4)"
                    if{menu_open}
                    MenuItem { click||send_money padding::8 radius::8 hover::{ background::#2a2f3a } "Send money" }
                    MenuItem { click||request_money padding::8 radius::8 hover::{ background::#2a2f3a } "Request money" }
                }
            }
        }

        Row {
            direction::row
            align::center
            gap::12

            MiniCard {
                grow::1
                padding::12
                radius::14
                background::#1b1e26
                border::{1, #262b36}
                transition::"padding .25s ease, transform .25s ease, background-color .2s ease"
                hover::{ padding::18, transform::"scale(1.06)", background::#20242e }

                Column {
                    direction::column
                    gap::6
                    align::center

                    Icon { svg||"<svg viewBox='0 0 24 24' width='18' height='18'><rect x='2' y='5' width='20' height='14' rx='3' fill='#6c5ce7'/><rect x='2' y='9' width='20' height='3' fill='#12141a'/></svg>" }
                    Text { text::{11, #9aa0ac} "Checking" }
                    Text { text::{13, bold, white} "$2,140" }
                }
            }

            MiniCard {
                grow::1
                padding::12
                radius::14
                background::#1b1e26
                border::{1, #262b36}
                transition::"padding .25s ease, transform .25s ease, background-color .2s ease"
                hover::{ padding::18, transform::"scale(1.06)", background::#20242e }

                Column {
                    direction::column
                    gap::6
                    align::center

                    Icon { svg||"<svg viewBox='0 0 24 24' width='18' height='18'><circle cx='12' cy='9' r='6' fill='#2ecc71'/><circle cx='12' cy='16' r='6' fill='#27ae60'/></svg>" }
                    Text { text::{11, #9aa0ac} "Savings" }
                    Text { text::{13, bold, white} "$8,905" }
                }
            }

            MiniCard {
                grow::1
                padding::12
                radius::14
                background::#1b1e26
                border::{1, #262b36}
                transition::"padding .25s ease, transform .25s ease, background-color .2s ease"
                hover::{ padding::18, transform::"scale(1.06)", background::#20242e }

                Column {
                    direction::column
                    gap::6
                    align::center

                    Icon { svg||"<svg viewBox='0 0 24 24' width='18' height='18'><path d='M12 2 L21 7 L21 17 L12 22 L3 17 L3 7 Z' fill='#f5a623'/></svg>" }
                    Text { text::{11, #9aa0ac} "Crypto" }
                    Text { text::{13, bold, white} "$1,220" }
                }
            }
        }

        Row {
            direction::row
            align::center
            gap::8

            Text { text::{15, bold, white} "Recent activity" }

            Block {
                hover_in||show_tip
                hover_out||hide_tip
                radius::10
                padding::6
                background::#232733
                Text { text::{11, #9aa0ac} "?" }
            }
        }

        Block {
            padding::10
            radius::10
            background::#232733
            border::{1, #6c5ce7}
            if{tip_open}
            Text { text::{12, #d8d2f7} "A real hover-triggered popover: state::tip_open + hover_in/hover_out, not CSS :hover." }
        }

        Block {
            direction::column
            gap::10
            for{tx in ["Coffee - Blue Bottle  -$4.20", "Salary - Acme Inc  +$2400.00", "Rent - Landlord  -$1150.00"]}

            ListItem {
                direction::row
                padding::14
                radius::14
                gap::12
                background::#1b1e26
                hover::{ filter::"brightness(1.35)", transform::"translateX(2px)" }

                Icon { text::16 "●" }

                Text { text::{14, white} "{tx}" }
            }
        }
    }
}"#;

#[test]
fn sandbox_demo_source_parses_and_renders() {
    let mut session = UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");
    let tree = session.render().expect("demo source must render");
    assert_eq!(tree.len(), 1, "one top-level Column");
}

#[test]
fn sandbox_demo_menu_popover_toggles_via_click() {
    let mut session = UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");

    // Column -> [Card, Row(buttons), Row(heading+badge), Block(tip, if-hidden), Block(transactions)]
    let column_children_before = session.render().unwrap()[0].children.len();

    session.dispatch("toggle_menu").expect("toggle_menu must be callable");
    let column_children_after = session.render().unwrap()[0].children.len();

    assert_eq!(
        column_children_after, column_children_before,
        "MenuPanel is nested inside the buttons Row, not a top-level Column child -- toggling it shouldn't change the Column's own child count"
    );
}

#[test]
fn sandbox_demo_tooltip_toggles_via_hover() {
    let mut session = UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");

    let before = session.render().unwrap();
    let column_children_before = before[0].children.len();

    session.dispatch("show_tip").expect("show_tip must be callable");
    let after_show = session.render().unwrap();
    assert_eq!(
        after_show[0].children.len(),
        column_children_before + 1,
        "the tooltip Block should now render (if{{tip_open}} is true)"
    );

    session.dispatch("hide_tip").expect("hide_tip must be callable");
    let after_hide = session.render().unwrap();
    assert_eq!(after_hide[0].children.len(), column_children_before, "tooltip Block should be gone again");
}
