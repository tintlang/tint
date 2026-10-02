# Overlays

Menus, dialogs and popovers are ordinary nodes with a few attributes. While such a node is on the
page it is an *overlay*:

```tn
fn toggle() { open = !open }
fn close() { open = false }

ui fn App() {
    state open = false
    Button { click||toggle ref||"menu_btn" "Menu" }
    Panel {
        dismiss||close anchor||"menu_btn" place||"bottom-start" trap||true
        role||"dialog" aria_label||"Menu"
        padding::8
        if{open}
        Button { click||close "One" }
        Button { click||close "Two" }
    }
}
```

| attribute | does |
|---|---|
| `dismiss||h` | runs `h` on Escape or a pointer press outside the node (the topmost overlay only; a press on its anchor does not count, so the anchor can toggle it) |
| `anchor||"name"` | keeps the node next to the node marked `ref||"name"`, `position: fixed`, following scroll and resize |
| `place||"bottom-start"` | `top`, `bottom`, `left` or `right`, then `-start` or `-end` (no suffix centres it). It flips to the other side when it would leave the window, and stays 8px inside it. Default `bottom-start` |
| `trap||true` | moves focus into the node, keeps Tab and Shift+Tab inside it, and gives focus back to what had it when the node goes away |

Attributes come before `if{}`. Style the layer as you like (`paint::{ background::.. }`,
a `z-index` in `layout::{ }`); the host only places it.
