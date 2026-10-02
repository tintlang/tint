# The `web` functions

Browser functions every Tint page can call, with no `js::` file. They are host functions like
the ones in [Callbacks and async](async.md): a function that returns a Promise needs `await` or a
trailing callback. A page function with the same name replaces the built-in one.

| function | does |
|---|---|
| `clipboard_write(text)` | copies text; `true` when the request was made |
| `sleep(ms)` | `await sleep(1500)` |
| `download(name, text)` | saves `text` as the file `name` |
| `open_url(url)` | opens a link in a new tab (`noopener`) |
| `set_title(text)` | sets the page title |
| `focus(ref)` | focuses the node marked `ref||"name"` |
| `scroll_to(ref)` | scrolls the node marked `ref||"name"` into view, smoothly |
| `fetch_text(url, callback)` | the response body as a string; an `Err` on a non-2xx status or a network error |
| `post_text(url, body, callback)` | the same with a POST and a text body |

```tn
async fn copy_install() {
    copy_label = "Copied"
    clipboard_write(install_cmd);
    await sleep(1500)
    copy_label = "Copy"
}

fn load() {
    fetch_text("/items.txt", |r: Result<string, string>| {
        match r {
            Ok { value } => { items = value },
            Err { error } => { status = "failed: {error}" }
        }
    })
}
```

They exist in the browser runtime only (`tint dev`, `tint build`, the sandbox); `tint run` in a
terminal does not provide them.
