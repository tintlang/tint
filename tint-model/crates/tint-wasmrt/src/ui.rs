//! The UI side of the runtime: the elements a compiled `ui fn` emits are
//! collected as `UiEvent`s and turned into render trees by the same
//! `IrRenderer` the interpreter path uses.

use super::*;
use tint_runtime::ui::ir_render::IrRenderer;

struct Ui {
    events: Vec<UiEvent>,
    renderer: IrRenderer,
    /// Between `ui_memo_begin` and `ui_memo_check` the scalars the app hands
    /// over are the memo key's, not an element's.
    memo_open: bool,
    memo_site: u32,
    memo_values: Vec<UiValue>,
    /// Identifies the theme tokens emitted so far in this run.
    tokens_epoch: u64,
}

static mut UI: *mut Ui = std::ptr::null_mut();

fn ui() -> &'static mut Ui {
    unsafe {
        if UI.is_null() {
            UI = Box::into_raw(Box::new(Ui {
                events: Vec::new(),
                renderer: IrRenderer::new(),
                memo_open: false,
                memo_site: 0,
                memo_values: Vec::new(),
                tokens_epoch: 0,
            }));
        }
        &mut *UI
    }
}

#[no_mangle]
pub extern "C" fn ui_open(template: u32) {
    ui().events.push(UiEvent::Open {
        template,
        values: Vec::new(),
    });
}

fn push_value(v: UiValue) {
    if ui().memo_open {
        ui().memo_values.push(v);
    } else if let Some(UiEvent::Open { values, .. }) = ui().events.last_mut() {
        values.push(v);
    }
}

#[no_mangle]
pub extern "C" fn ui_num(n: f64) {
    push_value(UiValue::Number(n));
}

#[no_mangle]
pub unsafe extern "C" fn ui_str(p: Ptr) {
    push_value(UiValue::Str(str_of(p).to_owned()));
}

#[no_mangle]
pub extern "C" fn ui_bool(b: u32) {
    push_value(UiValue::Bool(b != 0));
}

#[no_mangle]
pub extern "C" fn ui_close() {
    ui().events.push(UiEvent::Close);
}

#[no_mangle]
pub unsafe extern "C" fn ui_text(p: Ptr) {
    ui().events.push(UiEvent::Text(str_of(p).to_owned()));
}

#[no_mangle]
pub extern "C" fn ui_tokens(template: u32) {
    let ui = ui();
    ui.tokens_epoch = (ui.tokens_epoch ^ (template as u64 + 1)).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(23);
    ui.events.push(UiEvent::Tokens { template });
}

/// Starts a memo key: the scalars handed over until `ui_memo_check` are its inputs.
#[no_mangle]
pub extern "C" fn ui_memo_begin(site: u32) {
    let ui = ui();
    ui.memo_open = true;
    ui.memo_site = site;
    ui.memo_values.clear();
}

/// 1 when the renderer holds what the stretch emitted for this key last time
/// (the app then skips the stretch); else 0, and what the app emits up to
/// `ui_memo_end` is remembered under the key.
#[no_mangle]
pub extern "C" fn ui_memo_check() -> u32 {
    let ui = ui();
    ui.memo_open = false;
    let key = IrRenderer::memo_key(ui.memo_site, &ui.memo_values, ui.tokens_epoch);
    if ui.renderer.has_memo(key) {
        ui.events.push(UiEvent::MemoHit(key.0, key.1));
        1
    } else {
        ui.events.push(UiEvent::MemoStart(key.0, key.1));
        0
    }
}

#[no_mangle]
pub extern "C" fn ui_memo_end() {
    ui().events.push(UiEvent::MemoEnd);
}

/// Renders what has been emitted since the last call and returns the
/// top-level nodes as JSON (a new string; the caller releases it).
#[no_mangle]
pub extern "C" fn ui_render_json() -> Ptr {
    let ui = ui();
    let events = std::mem::take(&mut ui.events);
    ui.tokens_epoch = 0;
    let nodes = ui.renderer.render(&module().ui_templates, &events);
    let list: Vec<_> = nodes.iter().map(|n| (**n).clone()).collect();
    new_str(serde_json::to_string_pretty(&list).unwrap())
}

/// The tree of what has been emitted since the last call.
pub fn render_nodes() -> Vec<std::rc::Rc<tint_runtime::ui::render::UiRenderNode>> {
    let ui = ui();
    let events = std::mem::take(&mut ui.events);
    ui.tokens_epoch = 0;
    ui.renderer.render(&module().ui_templates, &events)
}

/// Drops what has been emitted (a render that failed half way).
pub fn discard_events() {
    let ui = ui();
    ui.events.clear();
    ui.tokens_epoch = 0;
    ui.memo_open = false;
}

/// The `app { }` metadata the compiler stored in the module.
pub fn app_meta_json() -> String {
    state().meta.clone()
}
