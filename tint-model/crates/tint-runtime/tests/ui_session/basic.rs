#[test]
fn state_starts_at_its_declared_initial_value() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].tag, "Button");
    assert_eq!(tree[0].on_click, Some("toggle_menu".to_string()));
}

#[test]
fn dispatching_the_click_handler_toggles_state_and_the_next_render_reflects_it() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    assert_eq!(session.dispatch("toggle_menu").expect("dispatch failed").len(), 2);
    assert_eq!(session.render().expect("render failed").len(), 2);
    assert_eq!(session.dispatch("toggle_menu").expect("dispatch failed").len(), 1);
}

#[test]
fn nested_helper_calls_share_persistent_ui_state() {
    let src = r#"
fn increment() { count = count + 1 }
fn on_click() { increment() }
ui fn App() {
    state count = 0
    Block { click||on_click
        Zero { if{count == 0} }
        One { if{count == 1} }
    }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    assert_eq!(session.render().expect("render failed")[0].children[0].tag, "Zero");
    session.dispatch("on_click").expect("dispatch failed");
    assert_eq!(session.render().expect("render failed")[0].children[0].tag, "One");
}

#[test]
fn nested_match_helper_can_run_from_a_frame_handler() {
    let src = r#"
fn reset_ball(direction) {
    ball_vx = direction
    serve_seed = serve_seed + 283
    if serve_seed > 1000 { serve_seed = serve_seed - 1000 }
    ball_vy = match serve_seed {
        seed if seed > 800 => 3,
        seed if seed > 600 => 2,
        seed if seed > 400 => 0 - 2,
        seed if seed > 200 => 0 - 1,
        _ => 1,
    }
}
fn on_frame(dt) { if started == 1 { reset_ball(1) } }
ui fn App() {
    state ball_vx = 1
    state ball_vy = 1
    state serve_seed = 417
    state started = 1
    Page { frame||on_frame }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    session
        .dispatch_with_args("on_frame", &[tint_evaluator::Value::Number(0.016)])
        .expect("frame dispatch failed");
}

#[test]
fn ui_session_rejects_assignment_to_immutable_local_before_runtime() {
    let src = r#"
fn broken() {
    let value = 1
    value = 2
}
ui fn App() { Text { "ok" } }
"#;
    let error = match UiSession::new(src, "App") {
        Ok(_) => panic!("immutable assignment must be rejected"),
        Err(error) => error,
    };
    assert!(error.contains("AssignToImmutable"), "unexpected error: {}", error);
}

#[test]
fn hover_in_and_hover_out_attributes_reach_the_render_tree() {
    let src = r#"
fn show_tip() { tip_open = true }
fn hide_tip() { tip_open = false }
ui fn App() {
    state tip_open = false
    Block { hover_in||show_tip hover_out||hide_tip "?" }
    Block { if{tip_open} "Tooltip text" }
}

"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].on_hover_enter, Some("show_tip".to_string()));
    assert_eq!(tree[0].on_hover_leave, Some("hide_tip".to_string()));
    assert_eq!(session.dispatch("show_tip").expect("dispatch failed").len(), 2);
    assert_eq!(session.dispatch("hide_tip").expect("dispatch failed").len(), 1);
}

#[test]
fn pointer_down_is_a_first_class_immediate_click_handler() {
    let src = r#"
fn pressed() {}
ui fn App() {
    Button { pointer_down||pressed "Press" }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");
    assert_eq!(tree[0].on_click, Some("pressed".to_string()));
}

#[test]
fn geometry_styles_keep_width_and_auto_offsets() {
    let src = r#"
ui fn App() {
    Ball { width::4 height::20 left::10 right::"auto" "•" }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");
    assert!(tree[0].style.contains(&("width".to_string(), "4px".to_string())));
    assert!(tree[0].style.contains(&("height".to_string(), "20px".to_string())));
    assert!(tree[0].style.contains(&("right".to_string(), "auto".to_string())));
}

#[test]
fn assets_keyboard_events_and_frame_handlers_reach_the_render_tree() {
    let src = r#"
fn on_key(key: string) { last_key = key }
fn on_key_up(key: string) { last_key = key }
fn tick(dt: f32) { elapsed = elapsed + dt }
ui fn App() {
    state last_key = ""
    state elapsed = 0
    GameRoot {
        key::"game"
        key_down||on_key
        key_up||on_key_up
        frame||tick
        Image { key::"pacman" asset||"/assets/pacman/pacman.png" sound||"/assets/pacman/waka.ogg" }
        Text { "{last_key} {elapsed}" }
    }
}
"#;
    let mut session = UiSession::new(src, "App").expect("session construction failed");
    let tree = session.render().expect("render failed");
    assert_eq!(tree[0].on_key_down, Some("on_key".to_string()));
    assert_eq!(tree[0].on_key_up, Some("on_key_up".to_string()));
    assert_eq!(tree[0].on_frame, Some("tick".to_string()));
    assert_eq!(tree[0].key, Some("game".to_string()));
    assert_eq!(tree[0].children[0].asset, Some("/assets/pacman/pacman.png".to_string()));
    assert_eq!(tree[0].children[0].key, Some("pacman".to_string()));
    assert_eq!(tree[0].children[0].sound, Some("/assets/pacman/waka.ogg".to_string()));
    session.dispatch_with_args("on_key", &[tint_evaluator::Value::String("ArrowLeft".into())]).expect("key dispatch failed");
    session.dispatch_with_args("tick", &[tint_evaluator::Value::Number(0.016)]).expect("frame dispatch failed");
}

#[test]
fn unknown_ui_fn_fails_construction_instead_of_panicking() {
    assert!(UiSession::new(SRC, "DoesNotExist").is_err());
}

#[test]
fn dispatching_an_undeclared_handler_reports_an_error_instead_of_crashing() {
    let mut session = UiSession::new(SRC, "App").expect("session construction failed");
    assert!(session.dispatch("not_a_real_function").is_err());
    assert_eq!(session.render().expect("render failed after a failed dispatch").len(), 1);
}

// The actual sandbox seed demo (SRC_UI_DEMO in
// sandbox/src/lib/fsStore.svelte.js) -- kept in sync by hand, not by a
