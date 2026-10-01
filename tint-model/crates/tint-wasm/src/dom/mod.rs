//! Direct-DOM rendering backend for `UiSession`.
//!
//! Each render is diffed in Rust against the retained previous tree (`Retained`),
//! and only real differences touch the DOM; subtrees that are the same `Rc` as
//! last time are skipped without being compared. Event closures are
//! intentionally leaked by `Closure::forget`, which is acceptable for the
//! sandbox but should be addressed before using this in a long-lived app.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element};

use tint_runtime::ui::render::UiRenderNode;
use tint_runtime::ui::style::breakpoint_threshold;
use tint_runtime::ui_session::SessionBackend;
#[cfg(feature = "interpreter")]
use tint_runtime::ui_session::UiSession as InnerSession;

/// Shared state a click/hover closure needs: the live session (`state`
/// lives inside it) and where in the DOM to rebuild. Cheap to clone
/// (it's an `Rc`) so every closure can hold its own copy.
struct Shared {
    session: RefCell<Box<dyn SessionBackend>>,
    container_id: String,
    /// Handle from `window.setTimeout`, used to debounce the `resize`
    /// listener bound in `bind_resize_listener` -- a resize storm
    /// (dragging a window edge) should trigger one rebuild after it
    /// settles, not one per event.
    resize_timeout: Cell<Option<i32>>,
    /// The `frame||handler` name from the most recently mounted tree, if
    /// any -- refreshed on every `mount_tree` call (see
    /// `find_frame_handler`) so the running rAF loop (see
    /// `start_frame_loop`) always dispatches whatever the CURRENT tree
    /// asked for, not whatever was true when the loop started.
    frame_handler: RefCell<Option<String>>,
    /// Whether `start_frame_loop` has already scheduled its
    /// `requestAnimationFrame` chain for this session. A `ui fn` declares
    /// `frame||` at most once in practice, and the loop just keeps
    /// rescheduling itself once started (see `start_frame_loop`), so this
    /// only needs to gate the FIRST scheduling.
    raf_active: Cell<bool>,
    key_down_handler: RefCell<Option<String>>,
    key_up_handler: RefCell<Option<String>>,
    key_down_bound: Cell<bool>,
    /// `pointer_move||`/`pointer_up||` handlers of the current tree, and
    /// whether a `pointer_start||` press is in progress.
    pointer_move_handler: RefCell<Option<String>>,
    pointer_up_handler: RefCell<Option<String>>,
    pointer_bound: Cell<bool>,
    dragging: Cell<bool>,
    /// A `Preview`'s own session: it must not touch the host page (title,
    /// body style, history) or take over the keyboard.
    nested: bool,
    /// Live `Preview` sessions by element id (see `sync_previews`).
    previews: RefCell<std::collections::HashMap<String, PreviewSlot>>,
    preview_seq: Cell<u32>,
    key_up_bound: Cell<bool>,
    /// The rAF timestamp (ms, from `performance.now()`) the loop last ran
    /// at, used to compute the real elapsed `dt` (seconds) handed to the
    /// `frame||` handler. `None` on the very first frame, which reports
    /// `dt = 0` rather than a bogus jump from page load.
    last_frame_time: Cell<Option<f64>>,
    /// Running `tick||`/`every||` intervals, reconciled against the
    /// mounted tree on every render (see `sync_tick_timers`).
    timers: RefCell<Vec<TickTimer>>,
    /// Host JS functions callable from `.tn`; re-registered on `reload`.
    natives: Vec<(String, tint_runtime::ui_session::NativeFn)>,
    /// The tree currently on screen and its DOM elements (see `Retained`).
    retained: RefCell<Option<Retained>>,
}

/// One `Preview` node: the source it shows and, while that source runs, its
/// session. `session` is `None` after an error (the element shows the message).
struct PreviewSlot {
    source: String,
    entry: String,
    session: Option<DomSession>,
}

/// One `setInterval` started for a `tick||handler` / `every||ms` pair.
struct TickTimer {
    handler: String,
    every_ms: i32,
    id: i32,
    /// Kept alive while the interval runs.
    callback: Closure<dyn FnMut()>,
}

/// Direct-DOM counterpart to `UiSession` in lib.rs. Wraps the exact same
/// `tint_runtime::ui_session::UiSession` (so `state`/`click||`/
/// `hover_in||`/`hover_out||` all behave identically), but instead of
/// handing JS a tree to render, this builds real DOM nodes into
/// `container_id` itself.
///
/// Construction never touches the DOM and can't fail loudly -- same
/// convention as `UiSession::new` (bad source / unknown `ui_fn_name` is
/// stored, not thrown). Call `rerender()` once after constructing to do
/// the first paint, exactly like `UiSession::new` + `.tree()`.
#[wasm_bindgen]
pub struct DomSession {
    shared: Option<Rc<Shared>>,
    init_error: Option<String>,
}

#[cfg(feature = "interpreter")]
include!("interop.rs");
include!("session.rs");
include!("helpers.rs");
include!("render.rs");
#[cfg(all(feature = "compiled", target_arch = "wasm32"))]
include!("compiled.rs");
