# HTTP

Tint exposes an explicit asynchronous boundary:

```tn
fn load_scores() {
    let request_id = http_get("/scores.json")
}

fn on_scores(request_id, status, body) {
    // validate body and update state here
}
```

`http_get(url)` returns a numeric request id and queues a `GET` request. A
host calls `take_http_requests()`, performs `fetch`, and delivers completion
with `dispatch_http("on_scores", id, status, body)`.

This is not yet a suspending `await`: Tint parses `async`/`await`, but the
current evaluator does not resume a VM continuation after a browser promise.
The queue plus callback contract keeps networking usable without pretending
that synchronous game logic can block on HTTP.
