# Async status

`async`/`await` is not part of the currently supported Tint runtime. Do not use
async handlers, futures, `throws`, or `try` inside UI code yet.

For current interactions, declare a normal logic function and connect it with a
named event attribute:

```tn
fn refresh() {}

ui fn App() {
    Button { click||refresh "Refresh" }
}
```

Async syntax remains outside the current language reference until the runtime
has a stable state-machine implementation.
