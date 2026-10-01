use std::collections::BTreeMap;

use tint_ir::typed::{self, *};

use crate::values::{from_val, to_val};
use crate::{module, new_str, release, retain, str_of, Ptr};

/// Host-facing state; exists before `rt_init`, so the host can load the
/// storage before the program's globals are initialized.
#[derive(Default)]
struct HostState {
    storage: BTreeMap<String, String>,
    http: Vec<(u64, String)>,
    next_http: u64,
}

static mut HOST: *mut HostState = std::ptr::null_mut();

fn host() -> &'static mut HostState {
    unsafe {
        if HOST.is_null() {
            HOST = Box::into_raw(Box::new(HostState {
                next_http: 1,
                ..Default::default()
            }));
        }
        &mut *HOST
    }
}

#[no_mangle]
pub unsafe extern "C" fn storage_get_or(key: Ptr, default: Ptr) -> Ptr {
    let value = host()
        .storage
        .get(str_of(key))
        .cloned()
        .unwrap_or_else(|| str_of(default).to_owned());
    new_str(value)
}

#[no_mangle]
pub unsafe extern "C" fn storage_set(key: Ptr, value: Ptr) {
    host()
        .storage
        .insert(str_of(key).to_owned(), str_of(value).to_owned());
}

#[no_mangle]
pub unsafe extern "C" fn storage_remove(key: Ptr) {
    host().storage.remove(str_of(key));
}

#[no_mangle]
pub extern "C" fn now_ms() -> f64 {
    unsafe { crate::host_now_ms() }
}

#[no_mangle]
pub unsafe extern "C" fn http_get(url: Ptr) -> f64 {
    let st = host();
    let id = st.next_http;
    st.next_http += 1;
    st.http.push((id, str_of(url).to_owned()));
    id as f64
}

/// The storage as a JSON object (a new string).
#[no_mangle]
pub extern "C" fn storage_snapshot_json() -> Ptr {
    new_str(serde_json::to_string(&host().storage).unwrap())
}

/// Adds the entries of a JSON object to the storage.
#[no_mangle]
pub unsafe extern "C" fn storage_hydrate_json(json: Ptr) {
    if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(str_of(json)) {
        host().storage.extend(map);
    }
}

/// Queued `http_get` requests as JSON `[{'id':1,'method':'GET','url':'..'}]` (a new string).
#[no_mangle]
pub extern "C" fn http_take_json() -> Ptr {
    let requests: Vec<_> = std::mem::take(&mut host().http)
        .into_iter()
        .map(|(id, url)| serde_json::json!({ "id": id, "method": "GET", "url": url }))
        .collect();
    new_str(serde_json::to_string(&requests).unwrap())
}

/// A new string with the UTF-8 text at `ptr` (for the host to pass as an argument).
#[no_mangle]
pub unsafe extern "C" fn str_new(ptr: *const u8, len: u32) -> Ptr {
    new_str(String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len as usize)).into_owned())
}

/// The storage entries (for the host to persist).
pub fn host_storage_snapshot() -> BTreeMap<String, String> {
    host().storage.clone()
}

/// Adds storage entries (the host's saved values), before or after the app runs.
pub fn host_storage_hydrate(values: impl IntoIterator<Item = (String, String)>) {
    host().storage.extend(values);
}

/// Queued `http_get` requests: `(id, url)`.
pub fn host_take_http() -> Vec<(u64, String)> {
    std::mem::take(&mut host().http)
}

/// A new runtime string with `text`, as the pointer a handler takes for a `string` parameter.
pub fn new_string_ptr(text: &str) -> u32 {
    new_str(text.to_owned()) as usize as u32
}

/// What the host answered to a call of one of its functions.
pub enum NativeReply {
    /// Returned this value (JSON).
    Value(String),
    /// Failed with this message.
    Error(String),
    /// Will answer the callback later.
    Pending,
}

type NativeHook = dyn Fn(&str, &str, Option<u32>) -> NativeReply;

static mut NATIVE_HOOK: Option<Box<NativeHook>> = None;

/// Installs what runs the page's host functions: `(name, arguments as a JSON
/// array, callback id)`. The callback id is given when the call has a callback.
pub fn set_native_hook(hook: Box<NativeHook>) {
    unsafe { NATIVE_HOOK = Some(hook) };
}

#[cfg(feature = "test-natives")]
fn test_natives(name: &str, args: &str, cb: Option<u32>) -> NativeReply {
    let args: serde_json::Value = serde_json::from_str(args).unwrap();
    let value = |v: serde_json::Value| NativeReply::Value(v.to_string());
    match name {
        "shout" => value(serde_json::json!(format!(
            "{}!",
            args[0].as_str().unwrap_or("").to_uppercase()
        ))),
        "sum_all" => value(serde_json::json!(args[0]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_f64()).sum::<f64>())
            .unwrap_or(0.0))),
        "make_point" => {
            value(serde_json::json!({ "__tint_struct": "Point", "x": args[0], "y": args[1] }))
        }
        "boom" => NativeReply::Error("boom from the host".into()),
        "wait" if cb.is_some() => NativeReply::Pending,
        "wait_now" if cb.is_some() => value(serde_json::json!(true)),
        _ => NativeReply::Error(format!("no host function `{name}`")),
    }
}

fn native_reply(name: &str, args: &str, cb: Option<u32>) -> NativeReply {
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(hook) = NATIVE_HOOK.as_ref() {
            return hook(name, args, cb);
        }
    }
    #[cfg(feature = "test-natives")]
    {
        return test_natives(name, args, cb);
    }
    #[allow(unreachable_code)]
    NativeReply::Error(format!("no host function `{name}`"))
}

/// Callbacks the host holds: `(closure, type of the Result it receives)`.
static mut CALLBACKS: Vec<Option<(Ptr, TyId)>> = Vec::new();

/// The `Result` a callback of type `ty` receives: `Ok(value)` (`message` is the JSON of the value)
/// or `Err(message)` (one owned reference).
fn result_object(ty: TyId, ok: bool, message: &str) -> Ptr {
    let m = module();
    let value;
    let outcome = if ok {
        value =
            serde_json::from_str::<serde_json::Value>(message).unwrap_or(serde_json::Value::Null);
        Ok(&value)
    } else {
        Err(message)
    };
    let v = typed::json::callback_result(&m.types, ty, outcome);
    from_val(m, ty, &v) as usize as Ptr
}

/// Calls the host function `index` (see `Module::natives`). Arguments are
/// passed like for `call`. With a `closure`, the host answers through it: an
/// answer that is ready is returned as the `Result` to deliver now.
#[no_mangle]
pub unsafe extern "C" fn host_native(
    index: u32,
    n: u32,
    args: *mut u64,
    tys: *const u32,
    dst_ty: u32,
    closure: Ptr,
    cb_ty: u32,
) -> u64 {
    let m = module();
    let name = &m.natives[index as usize];
    let mut list = Vec::new();
    for i in 0..n as usize {
        let ty = TyId(*tys.add(i));
        let v = to_val(m, ty, *args.add(i));
        match typed::json::val_to_json(&m.types, ty, &v) {
            Ok(j) => list.push(j),
            Err(e) => crate::fail(format!("{name}: {e}")),
        }
    }
    let json_args = serde_json::Value::Array(list).to_string();
    let cb = if closure.is_null() {
        None
    } else {
        retain(closure);
        #[allow(static_mut_refs)]
        let table = &mut CALLBACKS;
        table.push(Some((closure, TyId(cb_ty))));
        Some((table.len() - 1) as u32)
    };
    let reply = native_reply(name, &json_args, cb);
    match (cb, reply) {
        (None, NativeReply::Value(text)) => {
            let ty = TyId(dst_ty);
            let parsed =
                serde_json::from_str::<serde_json::Value>(&text).map_err(|e| e.to_string());
            match parsed.and_then(|j| typed::json::json_to_val(&m.types, ty, &j)) {
                Ok(v) => from_val(m, ty, &v),
                Err(e) => crate::fail(format!("{name}: {e}")),
            }
        }
        (None, NativeReply::Error(e)) => crate::fail(format!("{name}: {e}")),
        (None, NativeReply::Pending) => crate::fail(format!(
            "{name}: returns a Promise; pass a callback as the last argument"
        )),
        (Some(id), NativeReply::Pending) => {
            let _ = id;
            0
        }
        (Some(id), reply) => {
            #[allow(static_mut_refs)]
            let table = &mut CALLBACKS;
            if let Some((closure, ty)) = table[id as usize].take() {
                release(closure);
                match reply {
                    NativeReply::Value(text) => result_object(ty, true, &text) as usize as u64,
                    NativeReply::Error(e) => result_object(ty, false, &e) as usize as u64,
                    NativeReply::Pending => 0,
                }
            } else {
                0
            }
        }
    }
}

/// A callback the host kept, answered later: the closure and the `Result` to hand it.
pub fn take_callback(id: u32, ok: bool, message: &str) -> Option<(u32, u32)> {
    #[allow(static_mut_refs)]
    let table = unsafe { &mut CALLBACKS };
    let (closure, ty) = table.get_mut(id as usize)?.take()?;
    Some((
        closure as usize as u32,
        result_object(ty, ok, message) as usize as u32,
    ))
}

/// Gives back the reference `take_callback` handed out.
pub fn release_ptr(p: u32) {
    release(p as usize as Ptr);
}

/// For a host that answers a callback itself, writes the closure and Result to `out`.
#[no_mangle]
pub unsafe extern "C" fn callback_answer(id: u32, ok: u32, message: Ptr, out: *mut u32) -> u32 {
    let text = if message.is_null() {
        String::new()
    } else {
        str_of(message).to_owned()
    };
    match take_callback(id, ok != 0, &text) {
        Some((closure, result)) => {
            *out = closure;
            *out.add(1) = result;
            1
        }
        None => 0,
    }
}
