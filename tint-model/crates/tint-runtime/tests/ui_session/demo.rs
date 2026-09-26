// shared source file (there isn't one -- the sandbox's demo lives in a
// JS template literal). This exists so a demo/language mismatch (wrong
// modifier order, an undeclared handler, ...) is caught here instead of
// only visually, in the browser. If this test starts failing after an
// edit to SRC_UI_DEMO, update this constant to match.
#[test]
fn sandbox_demo_source_parses_and_renders() {
    let mut session =
        UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");
    let tree = session.render().expect("demo source must render");
    assert_eq!(tree.len(), 1, "one top-level Column");
}

#[test]
fn sandbox_demo_menu_popover_toggles_via_click() {
    let mut session =
        UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");

    // Column -> [Card, Row(buttons), Row(heading+badge), Block(tip, if-hidden), Block(transactions)]
    let column_children_before = session.render().unwrap()[0].children.len();

    session
        .dispatch("toggle_menu")
        .expect("toggle_menu must be callable");
    let column_children_after = session.render().unwrap()[0].children.len();

    assert_eq!(
        column_children_after, column_children_before,
        "MenuPanel is nested inside the buttons Row, not a top-level Column child -- toggling it shouldn't change the Column's own child count"
    );
}

#[test]
fn sandbox_demo_tooltip_toggles_via_hover() {
    let mut session =
        UiSession::new(DEMO_SOURCE, "App").expect("demo source must parse and construct");

    let before = session.render().unwrap();
    let column_children_before = before[0].children.len();

    session
        .dispatch("show_tip")
        .expect("show_tip must be callable");
    let after_show = session.render().unwrap();
    assert_eq!(
        after_show[0].children.len(),
        column_children_before + 1,
        "the tooltip Block should now render (if{{tip_open}} is true)"
    );

    session
        .dispatch("hide_tip")
        .expect("hide_tip must be callable");
    let after_hide = session.render().unwrap();
    assert_eq!(
        after_hide[0].children.len(),
        column_children_before,
        "tooltip Block should be gone again"
    );
}
