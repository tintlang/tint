// Browser functions every Tint page can call without a `js::` file (the `web` module):
//   clipboard_write(text)           copies text; true when the request was made
//   sleep(ms)                       `await sleep(1500)`
//   download(name, text)            saves text as a file
//   open_url(url)                   opens a link in a new tab
//   set_title(text)                 sets the page title
//   focus(ref) / scroll_to(ref)     a node marked `ref||"name"`
//   fetch_text(url)                 `await`: the response body, an error on a non-2xx status
//   query_get(name)                 the `?name=` value of the page's URL ("" when absent)
//   query_set(name, value)          sets it without a new history entry ("" removes it)
//   cache_get(key) / cache_set(key, value) / cache_fresh(key, ms)   page-lifetime string cache (`resource .. cache`)
//   post_text(url, body)            `await`: POST with a text body, same
// They are plain host functions (JSON in, JSON out, a Promise needs a callback or `await`), merged
// before the page's own, which win on a name clash. The checker knows the names (checker/names.rs).

thread_local! {
    /// `cache_*`: values the `resource ... cache` loaders keep for the life of the page.
    static WEB_CACHE: RefCell<std::collections::HashMap<String, (String, f64)>> = RefCell::new(std::collections::HashMap::new());
}

fn host_fn(name: &str, f: impl Fn(JsValue, JsValue) -> JsValue + 'static) -> (String, js_sys::Function) {
    let closure = Closure::wrap(Box::new(f) as Box<dyn Fn(JsValue, JsValue) -> JsValue>);
    let function: js_sys::Function = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
    closure.forget();
    (name.to_string(), function)
}

fn text_of(v: &JsValue) -> String {
    v.as_string().unwrap_or_default()
}

fn ref_element(name: &str) -> Option<web_sys::HtmlElement> {
    let selector = format!("[data-tint-ref=\"{}\"]", name.replace('\\', "\\\\").replace('"', "\\\""));
    document().ok()?.query_selector(&selector).ok()??.dyn_into().ok()
}

fn http_text(url: &str, post: Option<&str>) -> JsValue {
    let Some(window) = web_sys::window() else { return JsValue::UNDEFINED };
    let promise = match post {
        None => window.fetch_with_str(url),
        Some(body) => {
            let init = web_sys::RequestInit::new();
            init.set_method("POST");
            init.set_body(&JsValue::from_str(body));
            window.fetch_with_str_and_init(url, &init)
        }
    };
    js_sys::Promise::new(&mut |resolve, reject| {
        let reject_fetch = reject.clone();
        let on_response = Closure::once(move |response: JsValue| {
            let response: web_sys::Response = response.unchecked_into();
            if !response.ok() {
                let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&format!("HTTP {}", response.status())));
                return;
            }
            match response.text() {
                Ok(body) => drop(resolve.call1(&JsValue::UNDEFINED, &body)),
                Err(e) => drop(reject.call1(&JsValue::UNDEFINED, &e)),
            }
        });
        let on_failure = Closure::once(move |e: JsValue| drop(reject_fetch.call1(&JsValue::UNDEFINED, &e)));
        let _ = promise.then2(&on_response, &on_failure);
        on_response.forget();
        on_failure.forget();
    })
    .into()
}

fn web_natives() -> Vec<(String, js_sys::Function)> {
    vec![
        host_fn("clipboard_write", |text, _| {
            let Some(window) = web_sys::window() else { return JsValue::FALSE };
            let clipboard = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("clipboard")).unwrap_or(JsValue::UNDEFINED);
            let write = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText"))
                .ok()
                .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
            match write {
                Some(write) => {
                    // The Promise only reports a refused permission; the page has nothing to do about it.
                    if let Ok(p) = write.call1(&clipboard, &text) {
                        let swallow = Closure::once(|_: JsValue| {});
                        let _ = js_sys::Promise::from(p).catch(&swallow);
                        swallow.forget();
                    }
                    JsValue::TRUE
                }
                None => JsValue::FALSE,
            }
        }),
        host_fn("sleep", |ms, _| {
            let ms = ms.as_f64().unwrap_or(0.0).max(0.0) as i32;
            js_sys::Promise::new(&mut |resolve, _| {
                let done = Closure::once_into_js(move || {
                    let _ = resolve.call1(&JsValue::UNDEFINED, &JsValue::TRUE);
                });
                if let Some(window) = web_sys::window() {
                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(done.unchecked_ref(), ms);
                }
            })
            .into()
        }),
        host_fn("download", |name, text| {
            let parts = js_sys::Array::of1(&text);
            let options = web_sys::BlobPropertyBag::new();
            options.set_type("text/plain;charset=utf-8");
            let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else { return JsValue::FALSE };
            let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return JsValue::FALSE };
            if let Ok(a) = document().and_then(|d| d.create_element("a")) {
                if let Some(a) = a.dyn_ref::<web_sys::HtmlAnchorElement>() {
                    a.set_href(&url);
                    a.set_download(&text_of(&name));
                    a.click();
                }
            }
            let _ = web_sys::Url::revoke_object_url(&url);
            JsValue::TRUE
        }),
        host_fn("open_url", |url, _| {
            let opened = web_sys::window()
                .and_then(|w| w.open_with_url_and_target_and_features(&text_of(&url), "_blank", "noopener,noreferrer").ok());
            JsValue::from_bool(opened.is_some())
        }),
        host_fn("set_title", |text, _| {
            if let Ok(d) = document() {
                d.set_title(&text_of(&text));
            }
            JsValue::TRUE
        }),
        host_fn("focus", |name, _| match ref_element(&text_of(&name)) {
            Some(el) => {
                let _ = el.focus();
                JsValue::TRUE
            }
            None => JsValue::FALSE,
        }),
        host_fn("scroll_to", |name, _| match ref_element(&text_of(&name)) {
            Some(el) => {
                let options = web_sys::ScrollIntoViewOptions::new();
                options.set_behavior(web_sys::ScrollBehavior::Smooth);
                el.scroll_into_view_with_scroll_into_view_options(&options);
                JsValue::TRUE
            }
            None => JsValue::FALSE,
        }),
        host_fn("query_get", |name, _| {
            let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
            let params = web_sys::UrlSearchParams::new_with_str(&search).ok();
            JsValue::from_str(&params.and_then(|p| p.get(&text_of(&name))).unwrap_or_default())
        }),
        host_fn("query_set", |name, value| {
            let Some(window) = web_sys::window() else { return JsValue::FALSE };
            let Ok(url) = web_sys::Url::new(&window.location().href().unwrap_or_default()) else { return JsValue::FALSE };
            let params = url.search_params();
            let value = text_of(&value);
            if value.is_empty() {
                params.delete(&text_of(&name));
            } else {
                params.set(&text_of(&name), &value);
            }
            let target = format!("{}{}{}", url.pathname(), if params.to_string().as_string().unwrap_or_default().is_empty() { String::new() } else { format!("?{}", params.to_string().as_string().unwrap_or_default()) }, url.hash());
            let ok = window.history().and_then(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&target))).is_ok();
            JsValue::from_bool(ok)
        }),
        host_fn("cache_get", |key, _| JsValue::from_str(&WEB_CACHE.with(|c| c.borrow().get(&text_of(&key)).map(|(v, _)| v.clone()).unwrap_or_default()))),
        host_fn("cache_set", |key, value| {
            WEB_CACHE.with(|c| c.borrow_mut().insert(text_of(&key), (text_of(&value), now())));
            JsValue::TRUE
        }),
        host_fn("cache_fresh", |key, ms| {
            let limit = ms.as_f64().unwrap_or(0.0);
            JsValue::from_bool(WEB_CACHE.with(|c| c.borrow().get(&text_of(&key)).is_some_and(|(_, t)| now() - *t < limit)))
        }),
        host_fn("fetch_text", |url, _| http_text(&text_of(&url), None)),
        host_fn("post_text", |url, body| http_text(&text_of(&url), Some(&text_of(&body)))),
    ]
}
