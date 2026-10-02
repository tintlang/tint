// `enter::{...}`, `exit::{...}`, `view::{...}` and `layout::...` (inside `motion::{}`).
//
// None of them is CSS: the DOM host acts on them when elements come, go, scroll
// into view or move. They resolve into custom properties it reads when it builds
// or patches the node, like `drag` does:
//   --tint-fx-enter / -exit / -view : "prop:value|prop:value"  (the style to animate from / to / while in view)
//   --tint-fx-view-once             : "1"  (`view::{ once, ... }`: keep the in-view style once reached)
//   --tint-fx-layout                : "all" | "position" | "size"
// The animation itself is the node's own `transition` / `spring` (200ms ease if it has none).

fn apply_fx_style(name: &str, value: &UiModifierValue, out: &mut StyleList) {
    let mut list = Vec::new();
    apply_nested_style(value, &mut list);
    if let UiModifierValue::Tuple(items) = value {
        if name == "view" && items.iter().any(|i| matches!(i, UiModifierValue::Ident(w) if w == "once")) {
            out.push(("--tint-fx-view-once".to_string(), "1".to_string()));
        }
    }
    if list.is_empty() {
        return;
    }
    let encoded = merge_fx_list(&list)
        .iter()
        .map(|(k, v)| format!("{}:{}", k, v))
        .collect::<Vec<_>>()
        .join("|");
    out.push((format!("--tint-fx-{}", name), encoded));
}

/// Later entries win per property, as in a style block.
fn merge_fx_list(list: &StyleList) -> StyleList {
    let mut merged: StyleList = Vec::with_capacity(list.len());
    for (k, v) in list {
        match merged.iter_mut().find(|(e, _)| e == k) {
            Some(slot) => slot.1 = v.clone(),
            None => merged.push((k.clone(), v.clone())),
        }
    }
    merged
}

// `layout-id::"name"`: the node and the other node with the same name (in the previous or the next
// render) are one shared element: when it moves from one to the other it glides.
fn apply_fx_layout_id(value: &UiModifierValue, out: &mut StyleList) {
    if let UiModifierValue::Ident(s) | UiModifierValue::String(s) = value {
        out.push(("--tint-fx-layout-id".to_string(), s.clone()));
    }
}

// `layout::all` / `layout::position` / `layout::size`: when a re-render moves or
// resizes the node, it glides from where it was instead of jumping (FLIP).
fn apply_fx_layout(value: &UiModifierValue, out: &mut StyleList) {
    let mode = match value {
        UiModifierValue::Ident(s) | UiModifierValue::String(s) => match s.as_str() {
            "all" => "all",
            "position" => "position",
            "size" => "size",
            _ => return,
        },
        _ => return,
    };
    out.push(("--tint-fx-layout".to_string(), mode.to_string()));
}

// `scroll::{ from::{ opacity::0 } to::{ opacity::1 } }` and `scroll-page::{ .. }`: the style follows
// scroll. `scroll` goes from `from` to `to` while the node crosses the window (0 as its top enters
// at the bottom, 1 as its bottom leaves at the top); `scroll-page` follows the page's scroll
// (0 at the top, 1 at the end). Numbers in the two values are interpolated; the node should
// have no `transition` for the property.
//   --tint-fx-scroll-from / -to : "prop:value|prop:value"
//   --tint-fx-scroll-mode       : "element" | "page"
fn apply_fx_scroll(name: &str, value: &UiModifierValue, out: &mut StyleList) {
    let UiModifierValue::Tuple(items) = value else { return };
    for (side, key) in [("from", "--tint-fx-scroll-from"), ("to", "--tint-fx-scroll-to")] {
        let mut list = Vec::new();
        for item in items {
            if let UiModifierValue::MiniMod { key, value } = item {
                if key.len() == 1 && key[0] == side {
                    apply_nested_style(value, &mut list);
                }
            }
        }
        if list.is_empty() {
            continue;
        }
        let encoded = merge_fx_list(&list)
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect::<Vec<_>>()
            .join("|");
        out.push((key.to_string(), encoded));
    }
    out.push(("--tint-fx-scroll-mode".to_string(), if name == "scroll-page" { "page" } else { "element" }.to_string()));
}
