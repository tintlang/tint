//! `tint build` prerender: the first screen of the app as static HTML inside `#app`, so the page
//! has content (and text for crawlers) before the WebAssembly runtime has loaded. The runtime then
//! replaces it with the live DOM in one step, so nothing flashes.
//!
//! Only what a static page can show is written: elements, text, inline styles and attributes.
//! Hover, breakpoint and state styles, and everything that needs the runtime, wait for it.

use tint_runtime::ui::render::UiRenderNode;

/// The app's entry `ui fn` rendered once, or `None` when it cannot run at build time (it calls a
/// browser function while starting, say).
pub(crate) fn render(source: &str, entry: &str) -> Option<String> {
    let mut session = tint_runtime::ui_session::UiSession::new(source, entry).ok()?;
    let tree = session.render().ok()?;
    let mut out = String::new();
    for node in &tree {
        write_node(node, &mut out);
    }
    Some(out)
}

fn tag_name(node: &UiRenderNode) -> &'static str {
    if node.route.is_some() {
        "a"
    } else if node.tag == "TextArea" {
        "textarea"
    } else if node.tag == "Inline" || node.tag == "Text" {
        "span"
    } else if node.asset.is_some() && node.tag == "Image" {
        "img"
    } else if node.asset.is_some() && node.tag == "Audio" {
        "audio"
    } else if node.tag == "Button" || node.tag == "MenuItem" || node.on_click.is_some() || node.on_js.is_some() {
        "button"
    } else {
        "div"
    }
}

fn escape(text: &str, quotes: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quotes => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The inline style: later values win, `--tint-*` host flags stay out, and a node that animates in
/// when it scrolls into view starts in its final state (the page works without scripts).
fn style_text(node: &UiRenderNode) -> String {
    let revealed: Vec<String> = node
        .style
        .iter()
        .rev()
        .find(|(k, _)| k == "--tint-fx-view")
        .map(|(_, v)| v.split('|').filter_map(|p| p.split_once(':').map(|(k, _)| k.to_string())).collect())
        .unwrap_or_default();
    let mut merged: Vec<(&str, &str)> = Vec::new();
    for (k, v) in node.style.iter() {
        if k.starts_with("--tint-") || revealed.contains(k) {
            continue;
        }
        match merged.iter_mut().find(|(e, _)| e == k) {
            Some(slot) => slot.1 = v,
            None => merged.push((k, v)),
        }
    }
    merged.iter().map(|(k, v)| format!("{k}: {v};")).collect::<Vec<_>>().join(" ")
}

fn write_node(node: &UiRenderNode, out: &mut String) {
    let tag = tag_name(node);
    out.push('<');
    out.push_str(tag);
    let mut attr = |name: &str, value: &str| {
        out.push_str(&format!(" {name}=\"{}\"", escape(value, true)));
    };
    attr("data-tag", &node.tag);
    if let Some(key) = &node.key {
        attr("data-tint-key", key);
    }
    if let Some(route) = &node.route {
        attr("href", route);
        if let Some(target) = &node.target {
            attr("target", target);
            if target == "_blank" {
                attr("rel", "noopener noreferrer");
            }
        }
    }
    if let Some(asset) = &node.asset {
        attr("src", asset);
    }
    if let Some(reference) = &node.reference {
        attr("data-tint-ref", reference);
    }
    if let Some(class) = &node.class {
        attr("class", class);
    }
    for (name, value) in &node.attrs {
        attr(name, value);
    }
    let style = style_text(node);
    if !style.is_empty() {
        attr("style", &style);
    }
    out.push('>');
    if tag == "img" || tag == "audio" {
        if tag == "audio" {
            out.push_str("</audio>");
        }
        return;
    }
    if let Some(svg) = &node.svg {
        out.push_str(svg);
    } else if let Some(text) = &node.text {
        out.push_str(&escape(text, false));
    }
    for child in &node.children {
        write_node(child, out);
    }
    out.push_str(&format!("</{tag}>"));
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn renders_text_styles_and_escapes() {
        let html = render(
            "ui fn App() { state n = 1\n Column { padding::8 Label { \"a < b {n}\" } Button { click||inc \"go\" } } }\nfn inc() { n = n + 1 }",
            "App",
        )
        .expect("renders");
        assert!(html.contains("a &lt; b 1"), "{html}");
        assert!(html.contains("<button data-tag=\"Button\""), "{html}");
        assert!(html.contains("padding"), "{html}");
    }

    #[test]
    fn a_program_that_cannot_start_has_no_prerender() {
        assert!(render("ui fn App() { Label { \"x\" } }", "Missing").is_none());
    }
}
