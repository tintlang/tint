//! Browser side of `#[tint::export]`: each export becomes a `wasm_bindgen`
//! function `__tint_<name>(args: any[])`; `tint build` registers them as
//! natives. Values cross as plain JS data (numbers, strings, booleans, arrays,
//! plain objects; `undefined`/`null` is unit).

use wasm_bindgen::{JsCast, JsValue};

use crate::{NativeFn, Value};

pub fn call_native(native: (String, NativeFn), args: JsValue) -> Result<JsValue, JsValue> {
    let (name, function) = native;
    let array: js_sys::Array = args
        .dyn_into()
        .map_err(|_| JsValue::from_str(&format!("{name}: arguments must be an array")))?;
    let values = array
        .iter()
        .enumerate()
        .map(|(i, item)| {
            js_to_value(&item, 0).map_err(|e| JsValue::from_str(&format!("{name}: argument {}: {e}", i + 1)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = function(&values).map_err(|e| JsValue::from_str(&e.to_string()))?;
    value_to_js(&result).map_err(|e| JsValue::from_str(&format!("{name}: {e}")))
}

fn value_to_js(value: &Value) -> Result<JsValue, String> {
    use Value as V;
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
        // Structs and enums keep their identity as tagged objects (decoded back by
        // the runtime): `{ __tint_struct: "Name", ...fields }`,
        // `{ __tint_enum: "Result", variant: "Err", args: [..] }`.
        V::StructInstance { name, fields } => {
            let object = js_sys::Object::new();
            set(&object, "__tint_struct", JsValue::from_str(name))?;
            for (key, item) in fields {
                set(&object, key, value_to_js(item)?)?;
            }
            object.into()
        }
        V::EnumInstance { enum_name, variant, args } => {
            let object = js_sys::Object::new();
            set(&object, "__tint_enum", JsValue::from_str(enum_name))?;
            set(&object, "variant", JsValue::from_str(variant))?;
            let list = js_sys::Array::new();
            for item in args {
                list.push(&value_to_js(item)?);
            }
            set(&object, "args", list.into())?;
            object.into()
        }
        V::Callback(callback) => {
            let callback = callback.clone();
            let closure = wasm_bindgen::closure::Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(
                move |args: JsValue| {
                    let array: js_sys::Array = args.dyn_into().map_err(|_| JsValue::from_str("callback arguments"))?;
                    let values = array
                        .iter()
                        .map(|item| js_to_value(&item, 0))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| JsValue::from_str(&e))?;
                    let result = (callback.0)(&values).map_err(|e| JsValue::from_str(&e.to_string()))?;
                    value_to_js(&result).map_err(|e| JsValue::from_str(&e))
                },
            );
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

fn set(object: &js_sys::Object, key: &str, value: JsValue) -> Result<(), String> {
    js_sys::Reflect::set(object, &JsValue::from_str(key), &value)
        .map(|_| ())
        .map_err(|_| "cannot build object".to_string())
}

fn js_to_value(value: &JsValue, depth: u32) -> Result<Value, String> {
    use Value as V;
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
    if value.is_function() {
        let function: js_sys::Function = value.clone().into();
        return Ok(V::Callback(tint_evaluator::Callback(std::rc::Rc::new(move |args: &[Value]| {
            let js_args = js_sys::Array::new();
            for arg in args {
                js_args.push(&value_to_js(arg).map_err(crate::native_error_text)?);
            }
            let result = function
                .apply(&JsValue::UNDEFINED, &js_args)
                .map_err(|e| crate::native_error_text(format!("{e:?}")))?;
            js_to_value(&result, 0).map_err(crate::native_error_text)
        }))));
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
        let object: js_sys::Object = value.clone().into();
        if let Some(name) = js_sys::Reflect::get(&object, &JsValue::from_str("__tint_enum")).ok().and_then(|v| v.as_string()) {
            let variant = js_sys::Reflect::get(&object, &JsValue::from_str("variant")).ok().and_then(|v| v.as_string()).unwrap_or_default();
            let args = match js_sys::Reflect::get(&object, &JsValue::from_str("args")) {
                Ok(list) if js_sys::Array::is_array(&list) => js_sys::Array::from(&list)
                    .iter()
                    .map(|item| js_to_value(&item, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?,
                _ => Vec::new(),
            };
            return Ok(V::EnumInstance { enum_name: name, variant, args });
        }
        let struct_name = js_sys::Reflect::get(&object, &JsValue::from_str("__tint_struct")).ok().and_then(|v| v.as_string());
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
    Err("unsupported JS value".to_string())
}

/// An `async` export: `__tint_<name>(args)` returns a Promise (resolved with the
/// `Ok` value, rejected with the error message). The runtime turns it into
/// `Result::Ok/Err` for the Tint callback.
pub fn call_native_async(
    name: &'static str,
    args: JsValue,
    make: fn(Vec<Value>) -> crate::__private::NativeFuture,
) -> js_sys::Promise {
    let values = match args.dyn_into::<js_sys::Array>() {
        Ok(array) => array
            .iter()
            .enumerate()
            .map(|(i, item)| js_to_value(&item, 0).map_err(|e| format!("{name}: argument {}: {e}", i + 1)))
            .collect::<Result<Vec<_>, _>>(),
        Err(_) => Err(format!("{name}: arguments must be an array")),
    };
    wasm_bindgen_futures::future_to_promise(async move {
        let values = values.map_err(|e| JsValue::from_str(&e))?;
        match make(values).await {
            Ok(value) => value_to_js(&value).map_err(|e| JsValue::from_str(&format!("{name}: {e}"))),
            Err(message) => Err(JsValue::from_str(&message)),
        }
    })
}
