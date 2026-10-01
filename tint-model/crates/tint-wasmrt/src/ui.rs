//! The UI side of the runtime: the elements a compiled `ui fn` emits are
//! collected as `UiEvent`s and turned into render trees by the same
//! `IrRenderer` the interpreter path uses.

use super::*;
use tint_runtime::ui::ir_render::IrRenderer;

struct Ui {
    events: Vec<UiEvent>,
    renderer: IrRenderer,
}

static mut UI: *mut Ui = std::ptr::null_mut();

fn ui() -> &'static mut Ui {
    unsafe {
        if UI.is_null() {
            UI = Box::into_raw(Box::new(Ui {
                events: Vec::new(),
                renderer: IrRenderer::new(),
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
    if let Some(UiEvent::Open { values, .. }) = ui().events.last_mut() {
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
    ui().events.push(UiEvent::Tokens { template });
}

/// Renders what has been emitted since the last call and returns the
/// top-level nodes as JSON (a new string; the caller releases it).
#[no_mangle]
pub extern "C" fn ui_render_json() -> Ptr {
    let ui = ui();
    let events = std::mem::take(&mut ui.events);
    let nodes = ui.renderer.render(&module().ui_templates, &events);
    let list: Vec<_> = nodes.iter().map(|n| (**n).clone()).collect();
    new_str(serde_json::to_string_pretty(&list).unwrap())
}

/// The tree of what has been emitted since the last call.
pub fn render_nodes() -> Vec<std::rc::Rc<tint_runtime::ui::render::UiRenderNode>> {
    let ui = ui();
    let events = std::mem::take(&mut ui.events);
    ui.renderer.render(&module().ui_templates, &events)
}

/// Drops what has been emitted (a render that failed half way).
pub fn discard_events() {
    ui().events.clear();
}

/// The `app { }` metadata the compiler stored in the module.
pub fn app_meta_json() -> String {
    state().meta.clone()
}
