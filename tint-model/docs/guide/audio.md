# Audio

An event can request a short sound effect with `sound||`:

```tn
Button {
    sound||"/assets/pacman/waka.ogg"
    click||eat_pellet
    "Eat"
}
```

The browser backend creates a detached HTML audio element and calls `play()`
when the event fires. `Audio { asset||"..." }` is also rendered as an audio
element for longer-lived media. Audio pooling, volume buses, preload handles,
and streaming are intentionally deferred to the resource layer.
