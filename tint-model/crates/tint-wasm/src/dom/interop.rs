// JS interop: host JS functions callable from `.tn` as `name(...)`.
//
// The host (see packages/tint-runtime) loads the ES modules named by
// `app { js::"./x.js" }`, collects their exported functions into a plain JS
// object and hands it to `DomSession`'s constructor. Calls are synchronous;
// arguments and results cross as plain data (see `js_to_value`/`value_to_js`).

use tint_runtime::ui_session::NativeFn;

/// `{ js: [...], css: [...] }` declared by `app { ... }` in `source`, so the
/// host can load those files before constructing the session.
#[wasm_bindgen]
pub fn app_assets(source: &str) -> JsValue {
    let tokens = tint_lexer::collect_tokens(&mut tint_lexer::Lexer::new(source));
    match tint_parser::Parser::new(tokens).parse_program() {
        Ok(program) => assets_to_js(&program),
        Err(_) => no_assets(),
    }
}

#[cfg(feature = "bytecode")]
#[wasm_bindgen]
pub fn app_assets_bytecode(bytes: &[u8]) -> JsValue {
    match tint_runtime::bytecode::decode(bytes) {
        Ok(program) => assets_to_js(&program),
        Err(_) => no_assets(),
    }
}

#[derive(serde::Serialize)]
struct AssetsJson {
    js: Vec<String>,
    css: Vec<String>,
}

fn no_assets() -> JsValue {
    serde_wasm_bindgen::to_value(&AssetsJson { js: Vec::new(), css: Vec::new() }).unwrap_or(JsValue::NULL)
}

fn assets_to_js(program: &tint_ast::Program) -> JsValue {
    let meta = program.app_meta();
    serde_wasm_bindgen::to_value(&AssetsJson { js: meta.js, css: meta.css }).unwrap_or(JsValue::NULL)
}

/// Turns `{ name: function }` into natives. Non-function entries are ignored.
fn natives_from_js(object: Option<js_sys::Object>) -> Vec<(String, NativeFn)> {
    // The `web` functions first; a page function of the same name replaces one.
    let mut all: Vec<(String, js_sys::Function)> = web_natives();
    if let Some(object) = object {
        for entry in js_sys::Object::entries(&object).iter() {
            let entry: js_sys::Array = entry.into();
            let (Some(name), Ok(function)) = (entry.get(0).as_string(), entry.get(1).dyn_into::<js_sys::Function>()) else {
                continue;
            };
            all.retain(|(n, _)| *n != name);
            all.push((name, function));
        }
    }
    all.into_iter()
        .map(|(name, function)| {
            let label = name.clone();
            let native: NativeFn = Rc::new(move |args| call_js(&label, &function, args));
            (name, native)
        })
        .collect()
}

fn native_error(message: String) -> tint_evaluator::errors::EvalError {
    tint_evaluator::errors::EvalError::CallError {
        msg: message,
        span: tint_ast::Span::dummy(),
    }
}

fn call_js(
    name: &str,
    function: &js_sys::Function,
    args: &[tint_evaluator::Value],
) -> tint_evaluator::errors::EvalResult<tint_evaluator::Value> {
    // A trailing Tint callback is for an async JS function: it is not passed
    // to JS but receives `Result::Ok(value)` / `Result::Err(message)` once the
    // returned Promise settles.
    let (callback, args) = match args.split_last() {
        Some((tint_evaluator::Value::Callback(callback), rest)) => (Some(callback.clone()), rest),
        _ => (None, args),
    };
    let js_args = js_sys::Array::new();
    for arg in args {
        js_args.push(&value_to_js(arg).map_err(|e| native_error(format!("{name}: {e}")))?);
    }
    let result = function
        .apply(&JsValue::UNDEFINED, &js_args)
        .map_err(|e| {
            // Some evaluator paths drop call errors; make sure they are visible.
            let message = format!("{name}: {}", js_error_to_string(&e));
            web_sys::console::error_1(&JsValue::from_str(&format!("tint: JS function {message}")));
            native_error(message)
        })?;
    if result.is_instance_of::<js_sys::Promise>() {
        let Some(callback) = callback else {
            return Err(native_error(format!(
                "{name}: returns a Promise; pass a callback as the last argument: {name}(.., |r| ..)"
            )));
        };
        settle_into(name, js_sys::Promise::from(result), callback);
        return Ok(tint_evaluator::Value::Unit);
    }
    let value = js_to_value(&result, 0).map_err(|e| native_error(format!("{name}: {e}")))?;
    match callback {
        // Sync JS function with a callback: deliver `Ok(value)` at once.
        Some(callback) => {
            (callback.0)(&[result_value(Ok(value))])?;
            Ok(tint_evaluator::Value::Unit)
        }
        None => Ok(value),
    }
}

fn result_value(outcome: Result<tint_evaluator::Value, String>) -> tint_evaluator::Value {
    let (variant, payload) = match outcome {
        Ok(value) => ("Ok", value),
        Err(message) => ("Err", tint_evaluator::Value::String(message)),
    };
    tint_evaluator::Value::EnumInstance { enum_name: "Result".into(), variant: variant.into(), args: vec![payload] }
}

fn settle_into(name: &str, promise: js_sys::Promise, callback: tint_evaluator::Callback) {
    let deliver = move |outcome: Result<tint_evaluator::Value, String>| {
        if let Err(error) = (callback.0)(&[result_value(outcome)]) {
            web_sys::console::error_1(&JsValue::from_str(&format!("tint: async callback failed: {error}")));
        }
    };
    let deliver = Rc::new(deliver);
    let on_ok = {
        let deliver = deliver.clone();
        let label = name.to_string();
        Closure::once(move |value: JsValue| {
            deliver(js_to_value(&value, 0).map_err(|e| format!("{label}: {e}")));
        })
    };
    let on_err = Closure::once(move |error: JsValue| deliver(Err(js_error_to_string(&error))));
    let _ = promise.then2(&on_ok, &on_err);
    on_ok.forget();
    on_err.forget();
}

fn value_to_js(value: &tint_evaluator::Value) -> Result<JsValue, String> {
    use tint_evaluator::Value as V;
    Ok(match value {
        V::Number(n) | V::F64(n) => JsValue::from_f64(*n),
        V::F32(n) => JsValue::from_f64(*n as f64),
        V::I32(n) => JsValue::from_f64(*n as f64),
        V::I64(n) => JsValue::from_f64(*n as f64),
        V::U32(n) => JsValue::from_f64(*n as f64),
        V::U64(n) => JsValue::from_f64(*n as f64),
        V::U8(n) => JsValue::from_f64(*n as f64),
        V::String(s) => JsValue::from_str(s),
        V::Bool(b) => JsValue::from_bool(*b),
        V::Unit => JsValue::UNDEFINED,
        V::List(items) | V::Tuple(items) => {
            let array = js_sys::Array::new();
            for item in items {
                array.push(&value_to_js(item)?);
            }
            array.into()
        }
        V::Map(map) => {
            let object = js_sys::Object::new();
            for (key, item) in map {
                js_sys::Reflect::set(&object, &JsValue::from_str(key), &value_to_js(item)?)
                    .map_err(|_| "cannot build object".to_string())?;
            }
            object.into()
        }
        V::StructInstance { name, fields } => {
            let object = js_sys::Object::new();
            js_sys::Reflect::set(&object, &JsValue::from_str("__tint_struct"), &JsValue::from_str(name))
                .map_err(|_| "cannot build object".to_string())?;
            for (key, item) in fields {
                js_sys::Reflect::set(&object, &JsValue::from_str(key), &value_to_js(item)?)
                    .map_err(|_| "cannot build object".to_string())?;
            }
            object.into()
        }
        V::EnumInstance { enum_name, variant, args } => {
            let object = js_sys::Object::new();
            let list = js_sys::Array::new();
            for item in args {
                list.push(&value_to_js(item)?);
            }
            for (key, item) in [
                ("__tint_enum", JsValue::from_str(enum_name)),
                ("variant", JsValue::from_str(variant)),
                ("args", list.into()),
            ] {
                js_sys::Reflect::set(&object, &JsValue::from_str(key), &item)
                    .map_err(|_| "cannot build object".to_string())?;
            }
            object.into()
        }
        V::Callback(callback) => {
            let callback = callback.clone();
            let closure = Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(move |args: JsValue| {
                let array: js_sys::Array = args.dyn_into().map_err(|_| JsValue::from_str("callback arguments"))?;
                let values = array
                    .iter()
                    .map(|item| js_to_value(&item, 0))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| JsValue::from_str(&e))?;
                let result = (callback.0)(&values).map_err(|e| JsValue::from_str(&e.to_string()))?;
                value_to_js(&result).map_err(|e| JsValue::from_str(&e))
            });
            let spread = js_sys::Function::new_with_args("cb", "return (...args) => cb(args)");
            let function = spread
                .call1(&JsValue::UNDEFINED, closure.as_ref())
                .map_err(|_| "cannot create a JS function".to_string())?;
            closure.forget();
            function
        }
        _ => return Err("this Tint value cannot be passed to JS".to_string()),
    })
}

fn js_to_value(value: &JsValue, depth: u32) -> Result<tint_evaluator::Value, String> {
    use tint_evaluator::Value as V;
    if depth > 32 {
        return Err("value nested too deeply".to_string());
    }
    if value.is_undefined() || value.is_null() {
        return Ok(V::Unit);
    }
    if let Some(n) = value.as_f64() {
        return Ok(V::Number(n));
    }
    if let Some(s) = value.as_string() {
        return Ok(V::String(s));
    }
    if let Some(b) = value.as_bool() {
        return Ok(V::Bool(b));
    }
    if value.is_instance_of::<js_sys::Promise>() {
        return Err("a Promise cannot be used as a value; pass a callback to the function instead".to_string());
    }
    if js_sys::Array::is_array(value) {
        let array: js_sys::Array = value.clone().into();
        return array
            .iter()
            .map(|item| js_to_value(&item, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(V::List);
    }
    if value.is_object() && !value.is_function() {
        // Plain data only: DOM nodes, class instances etc. would silently
        // become empty maps otherwise.
        let proto = js_sys::Object::get_prototype_of(value);
        let plain = proto.is_null()
            || js_sys::Object::get_prototype_of(&proto).is_null();
        if plain {
            let object: js_sys::Object = value.clone().into();
            let get = |key: &str| js_sys::Reflect::get(&object, &JsValue::from_str(key)).ok();
            if let Some(name) = get("__tint_enum").and_then(|v| v.as_string()) {
                let variant = get("variant").and_then(|v| v.as_string()).unwrap_or_default();
                let args = match get("args") {
                    Some(list) if js_sys::Array::is_array(&list) => js_sys::Array::from(&list)
                        .iter()
                        .map(|item| js_to_value(&item, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => Vec::new(),
                };
                return Ok(V::EnumInstance { enum_name: name, variant, args });
            }
            let struct_name = get("__tint_struct").and_then(|v| v.as_string());
            let mut map = std::collections::HashMap::new();
            for entry in js_sys::Object::entries(&object).iter() {
                let entry: js_sys::Array = entry.into();
                if let Some(key) = entry.get(0).as_string() {
                    if struct_name.is_some() && key == "__tint_struct" {
                        continue;
                    }
                    map.insert(key, js_to_value(&entry.get(1), depth + 1)?);
                }
            }
            if let Some(name) = struct_name {
                return Ok(V::StructInstance { name, fields: map.into_iter().collect() });
            }
            return Ok(V::Map(map));
        }
    }
    Err("JS returned a value Tint cannot represent (function or non-plain object)".to_string())
}
