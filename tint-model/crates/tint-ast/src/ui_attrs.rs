//! Which node attributes become HTML attributes, shared by every tree builder.

pub const VALUE_ATTRS: &[&str] = &[
    "placeholder", "disabled", "readonly", "title", "alt", "tabindex", "props", "deps", "keys", "every", "anchor", "place", "trap", "role", "autofocus",
    "aria_label", "aria_labelledby", "aria_describedby", "aria_hidden", "aria_expanded", "aria_selected",
    "aria_checked", "aria_pressed", "aria_current", "aria_live", "aria_controls", "aria_modal", "aria_busy",
    "aria_disabled", "aria_haspopup", "aria_valuenow", "aria_valuemin", "aria_valuemax", "aria_invalid",
    "aria_required", "aria_orientation",
];

/// Handler attributes the host runs: `mount||h`, `unmount||h`, `effect||h` (on mount and whenever
/// `deps||{..}` changes), `dismiss||h` (Escape or a click outside the node), `pan||h`, `swipe||h`, `long_press||h` (gestures), `hotkey||h` with `keys||"mod+k"` (keyboard shortcut), `blur||h` (focus left the node), `poll||h` with `every||ms` (repeat while mounted), `sortable||h` (children can be dragged into a new order: `h(from, to)`), `scroll||h` (`h(top, height)` of a scrolled node). They reach the DOM as `data-tint-*`.
pub const LIFECYCLE_ATTRS: &[&str] = &["mount", "unmount", "effect", "dismiss", "pan", "swipe", "long_press", "hotkey", "blur", "poll", "sortable", "scroll"];

/// The HTML attribute a node attribute becomes: `aria_label` is `aria-label`, `mount` is
/// `data-tint-mount`, the rest keep their name.
pub fn html_attr_name(name: &str) -> String {
    if LIFECYCLE_ATTRS.contains(&name) || matches!(name, "deps" | "keys" | "every" | "anchor" | "place" | "trap") {
        format!("data-tint-{name}")
    } else {
        name.replace('_', "-")
    }
}

