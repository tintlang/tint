// A compiled app (`tint-wasmgen` output) as a session backend.
//
// The app is a separate wasm module that imports this module's memory and
// runtime functions (`tint-wasmrt`, linked in here). The host instantiates it and
// hands its exports over; calling a handler is calling an export, and the UI
// the app emits arrives through the runtime's event buffer, which is turned
// into the same render nodes the interpreter produces.

use std::collections::HashMap;

use tint_evaluator::Value as EvalValue;
use tint_runtime::vm::HttpRequest;

struct CompiledSession {
    exports: js_sys::Object,
    ui_fn: String,
    meta: tint_ast::AppMeta,
}

impl CompiledSession {
    fn call(&self, name: &str, args: &[JsValue]) -> Result<JsValue, String> {
        let function = js_sys::Reflect::get(&self.exports, &JsValue::from_str(name))
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
            .ok_or_else(|| format!("the app has no `{name}`"))?;
        let list = js_sys::Array::new();
        for arg in args {
            list.push(arg);
        }
        function.apply(&JsValue::UNDEFINED, &list).map_err(|e| {
            let message = e
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
                .unwrap_or_else(|| js_error_to_string(&e));
            message
        })
    }
}

impl SessionBackend for CompiledSession {
    fn render(&mut self) -> Result<Vec<Rc<UiRenderNode>>, String> {
        tint_wasmrt::ui::discard_events();
        if let Err(e) = self.call(&self.ui_fn.clone(), &[]) {
            tint_wasmrt::ui::discard_events();
            return Err(e);
        }
        Ok(tint_wasmrt::ui::render_nodes())
    }

    fn dispatch_with_args(&mut self, handler: &str, args: &[EvalValue]) -> Result<Vec<Rc<UiRenderNode>>, String> {
        let sig = tint_wasmrt::export_sig(handler).unwrap_or_default();
        let mut js_args = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let integer = sig.as_bytes().get(i) == Some(&b'i');
            js_args.push(match arg {
                EvalValue::Number(n) | EvalValue::F64(n) if integer => js_sys::BigInt::from(*n as i64).into(),
                EvalValue::Number(n) | EvalValue::F64(n) => JsValue::from_f64(*n),
                EvalValue::String(s) => JsValue::from_f64(tint_wasmrt::new_string_ptr(s) as f64),
                EvalValue::Bool(b) => JsValue::from_f64(*b as u8 as f64),
                other => return Err(format!("a compiled handler cannot take {other:?}")),
            });
        }
        self.call(handler, &js_args)?;
        self.render()
    }

    fn answer_callback(&mut self, id: u32, ok: bool, message: &str) -> Result<Vec<Rc<UiRenderNode>>, String> {
        let Some((closure, result)) = tint_wasmrt::take_callback(id, ok, message) else { return Ok(Vec::new()) };
        let ran = self.call("tint:run_callback", &[JsValue::from_f64(closure as f64), JsValue::from_f64(result as f64)]);
        tint_wasmrt::release_ptr(closure);
        ran?;
        self.render()
    }

    fn call_value(&mut self, _function: EvalValue, _args: &[EvalValue]) -> Result<Vec<Rc<UiRenderNode>>, String> {
        Err("callbacks from host functions are not supported by compiled apps yet".to_string())
    }

    fn set_viewport_width(&mut self, width: f64) {
        let _ = self.call("tint:set_viewport_width", &[JsValue::from_f64(width)]);
    }

    fn set_route_path(&mut self, path: &str) {
        let _ = self.call("tint:set_route_path", &[JsValue::from_f64(tint_wasmrt::new_string_ptr(path) as f64)]);
    }

    fn app_meta(&self) -> &tint_ast::AppMeta {
        &self.meta
    }

    fn page_style(&self) -> Vec<(String, String)> {
        tint_runtime::ui::style::resolve_page_style(&self.meta.page)
    }

    fn app_css(&self) -> String {
        tint_runtime::ui::style::app_at_rules(&self.meta)
    }

    fn switch_ui_fn(&mut self, name: &str) -> Result<(), String> {
        if name == self.ui_fn {
            return Ok(());
        }
        self.call(&format!("tint:enter:{name}"), &[])?;
        self.ui_fn = name.to_string();
        Ok(())
    }

    fn storage_snapshot(&self) -> HashMap<String, String> {
        tint_wasmrt::host_storage_snapshot().into_iter().collect()
    }

    fn hydrate_storage(&mut self, values: HashMap<String, String>) {
        tint_wasmrt::host_storage_hydrate(values);
    }

    fn take_http_requests(&mut self) -> Vec<HttpRequest> {
        tint_wasmrt::host_take_http()
            .into_iter()
            .map(|(id, url)| HttpRequest { id, method: "GET".to_string(), url })
            .collect()
    }
}

thread_local! {
    static COMPILED_SHARED: RefCell<std::rc::Weak<Shared>> = RefCell::new(std::rc::Weak::new());
}

/// Delivers a settled Promise to the callback that waits for it, then re-renders.
fn deliver_native(id: u32, ok: bool, message: String) {
    let Some(shared) = COMPILED_SHARED.with(|s| s.borrow().upgrade()) else { return };
    let tree = match shared.session.try_borrow_mut() {
        Ok(mut session) => session.answer_callback(id, ok, &message),
        Err(_) => Err("session is busy".to_string()),
    };
    match tree {
        Ok(tree) if tree.is_empty() => {}
        Ok(tree) => {
            if let Err(e) = mount_tree(tree, &shared) {
                web_sys::console::error_1(&e);
            }
        }
        Err(e) => web_sys::console::error_1(&JsValue::from_str(&format!("tint: async callback failed: {e}"))),
    }
}

/// Runs the page's JS functions for the runtime: arguments and results as JSON.
fn install_native_hook(natives: Vec<(String, js_sys::Function)>) {
    let natives: HashMap<String, js_sys::Function> = natives.into_iter().collect();
    tint_wasmrt::set_native_hook(Box::new(move |name, args, cb| {
        let Some(function) = natives.get(name) else {
            return tint_wasmrt::NativeReply::Error(format!("no host function `{name}`"));
        };
        let list = match js_sys::JSON::parse(args) {
            Ok(v) => js_sys::Array::from(&v),
            Err(_) => return tint_wasmrt::NativeReply::Error("bad arguments".into()),
        };
        let result = match js_sys::Reflect::apply(function, &JsValue::UNDEFINED, &list) {
            Ok(v) => v,
            Err(e) => {
                let message = js_error_to_string(&e);
                web_sys::console::error_1(&JsValue::from_str(&format!("tint: JS function {name}: {message}")));
                return tint_wasmrt::NativeReply::Error(message);
            }
        };
        if result.is_instance_of::<js_sys::Promise>() {
            let Some(id) = cb else {
                return tint_wasmrt::NativeReply::Error("returns a Promise; pass a callback as the last argument".into());
            };
            let on_ok = Closure::once(move |value: JsValue| {
                let json = js_sys::JSON::stringify(&value).ok().and_then(|s| s.as_string()).unwrap_or_else(|| "null".into());
                deliver_native(id, true, json);
            });
            let on_err = Closure::once(move |error: JsValue| deliver_native(id, false, js_error_to_string(&error)));
            let _ = js_sys::Promise::from(result).then2(&on_ok, &on_err);
            on_ok.forget();
            on_err.forget();
            return tint_wasmrt::NativeReply::Pending;
        }
        let json = js_sys::JSON::stringify(&result).ok().and_then(|s| s.as_string()).unwrap_or_else(|| "null".into());
        tint_wasmrt::NativeReply::Value(json)
    }));
}

/// Prepares the runtime for the app: loads the saved values and installs the page's host
/// functions (`natives`, an object of functions). Call it after the runtime is instantiated and
/// before the app is: the app's `state` initializers may read the values and call the functions.
#[wasm_bindgen]
pub fn compiled_prepare(natives: Option<js_sys::Object>) {
    tint_wasmrt::host_storage_hydrate(load_browser_storage());
    install_native_hook(
        js_sys::Object::entries(&natives.unwrap_or_default())
            .iter()
            .filter_map(|e| {
                let e: js_sys::Array = e.into();
                Some((e.get(0).as_string()?, e.get(1).dyn_into::<js_sys::Function>().ok()?))
            })
            .collect(),
    );
}

#[wasm_bindgen]
impl DomSession {
    /// A session over a compiled app: `exports` are the exports of the
    /// instantiated app module, `ui_fn_name` the `ui fn` to render first.
    pub fn from_compiled(exports: js_sys::Object, ui_fn_name: &str, container_id: &str) -> DomSession {
        dom_tree_shape();
        let meta: Result<tint_ast::AppMeta, String> = {
            let json = tint_wasmrt::ui::app_meta_json();
            if json.is_empty() {
                Ok(Default::default())
            } else {
                serde_json::from_str(&json).map_err(|e| format!("bad app metadata: {e}"))
            }
        };
        let session = meta.map(|meta| {
            let session = CompiledSession { exports, ui_fn: ui_fn_name.to_string(), meta };
            // States of different `ui fn`s may share a name: start the page's own.
            let _ = session.call(&format!("tint:enter:{ui_fn_name}"), &[]);
            Box::new(session) as Box<dyn SessionBackend>
        });
        let dom = Self::from_inner(session, container_id, false, Vec::new());
        if let Some(shared) = &dom.shared {
            COMPILED_SHARED.with(|s| *s.borrow_mut() = Rc::downgrade(shared));
        }
        dom
    }
}
