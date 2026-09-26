#[test]
fn state_starts_at_its_declared_initial_value() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");

    // [Button, MenuPanel-if-check]: MenuPanel is absent because
    // menu_open starts false.
    assert_eq!(
        tree.len(),
        1,
        "menu_open starts false, so MenuPanel shouldn't render at all"
    );
    assert_eq!(tree[0].tag, "Button");
    assert_eq!(
        tree[0].on_click,
        Some("toggle_menu".to_string()),
        "click||toggle_menu should reach the render tree"
    );
}

#[test]
fn dispatching_the_click_handler_toggles_state_and_the_next_render_reflects_it() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");

    let after_click = session.dispatch("toggle_menu").expect("dispatch failed");
    assert_eq!(
        after_click.len(),
        2,
        "menu_open is now true, so MenuPanel should render"
    );
    assert_eq!(after_click[1].tag, "MenuPanel");

    // Independently re-rendering (no dispatch) must show the SAME state
    // -- proving it's real persistent state, not something dispatch()
    // computed only for its own return value.
    let rendered_again = session.render().expect("render failed");
    assert_eq!(
        rendered_again.len(),
        2,
        "state must still read true on a later render(), not just right after dispatch()"
    );

    // A second click flips it back off.
    let after_second_click = session.dispatch("toggle_menu").expect("dispatch failed");
    assert_eq!(
        after_second_click.len(),
        1,
        "second click should toggle menu_open back to false"
    );
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
    assert_eq!(
        after_hover_in.len(),
        2,
        "hovering in should reveal the tooltip block"
    );
    assert_eq!(after_hover_in[1].tag, "Block");

    let after_hover_out = session.dispatch("hide_tip").expect("dispatch failed");
    assert_eq!(
        after_hover_out.len(),
        1,
        "hovering out should hide it again"
    );
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
    assert!(
        err.is_err(),
        "an unknown handler should come back as Err, not panic the session"
    );

    // The session must still be usable afterwards.
    let tree = session
        .render()
        .expect("render failed after a failed dispatch");
    assert_eq!(tree.len(), 1);
}

// The actual sandbox seed demo (SRC_UI_DEMO in
// sandbox/src/lib/fsStore.svelte.js) -- kept in sync by hand, not by a
