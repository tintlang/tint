use super::item::Item;
use crate::{AttributeList, UiModifierValue};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Program {
    pub globals: AttributeList,
    pub items: Vec<Item>,
}

/// Page metadata from `app { title::"...", lang::"..." }`.
#[derive(Debug, Clone, Default)]
pub struct AppMeta {
    pub title: Option<String>,
    pub lang: Option<String>,
    /// `router::on`: the browser host handles in-app links (`route||"/x"`)
    /// itself -- history navigation, no page load -- and keeps `route_path`
    /// current. Without it, links are ordinary links.
    pub router: bool,
    /// `page::{ margin::0, background::#000 }`: styles for the host page's
    /// `<body>`, in the same modifier syntax as a node's `layout::{ ... }`.
    pub page: Vec<UiModifierValue>,
    /// `keyframes.name::{ from::{ ... } p50::{ ... } to::{ ... } }`: each
    /// frame is its selector (`from`, `to`, `p50` = 50%) and its style items.
    pub keyframes: Vec<(String, Vec<(String, Vec<UiModifierValue>)>)>,
    /// `font.Family::{ src::"/f.woff2", weight::400 }`: one `@font-face` per
    /// declaration, as its property items.
    pub fonts: Vec<(String, Vec<UiModifierValue>)>,
    /// `route.Landing::"/"` or `route.Pong::{ path::"/pong", title::"Pong" }`:
    /// which `ui fn` the browser host renders for which URL path.
    pub routes: Vec<RouteDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteDecl {
    pub ui_fn: String,
    pub path: String,
    pub title: Option<String>,
}

impl Program {
    /// Reads every `app { ... }` declaration; a later one overrides an
    /// earlier one key by key. Unknown keys are ignored.
    pub fn app_meta(&self) -> AppMeta {
        let mut meta = AppMeta::default();
        for item in &self.items {
            let Item::App(app) = item else { continue };
            for modifier in &app.modifiers {
                if let ("page", UiModifierValue::Tuple(items)) =
                    (modifier.path.join(".").as_str(), &modifier.value)
                {
                    meta.page.extend(items.iter().cloned());
                    continue;
                }
                let path = modifier.path.join(".");
                if let Some(ui_fn) = path.strip_prefix("route.") {
                    let mut decl = RouteDecl { ui_fn: ui_fn.to_string(), path: String::new(), title: None };
                    match &modifier.value {
                        UiModifierValue::String(p) => decl.path = p.clone(),
                        UiModifierValue::Tuple(items) => {
                            for item in items {
                                let UiModifierValue::MiniMod { key, value } = item else { continue };
                                let UiModifierValue::String(text) = &**value else { continue };
                                match key.join(".").as_str() {
                                    "path" => decl.path = text.clone(),
                                    "title" => decl.title = Some(text.clone()),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    if decl.path.starts_with('/') {
                        meta.routes.retain(|r| r.ui_fn != decl.ui_fn);
                        meta.routes.push(decl);
                    }
                    continue;
                }
                if let (Some(name), UiModifierValue::Tuple(items)) =
                    (path.strip_prefix("keyframes."), &modifier.value)
                {
                    let frames = items
                        .iter()
                        .filter_map(|item| {
                            let UiModifierValue::MiniMod { key, value } = item else { return None };
                            let body = match &**value {
                                UiModifierValue::Tuple(v) => v.clone(),
                                other => vec![other.clone()],
                            };
                            Some((key.join("."), body))
                        })
                        .collect();
                    meta.keyframes.push((name.to_string(), frames));
                    continue;
                }
                if let (Some(family), UiModifierValue::Tuple(items)) =
                    (path.strip_prefix("font."), &modifier.value)
                {
                    meta.fonts.push((family.to_string(), items.clone()));
                    continue;
                }
                let (UiModifierValue::String(value) | UiModifierValue::Ident(value)) =
                    &modifier.value
                else {
                    continue;
                };
                match modifier.path.join(".").as_str() {
                    "title" => meta.title = Some(value.clone()),
                    "lang" => meta.lang = Some(value.clone()),
                    "router" => meta.router = value == "on",
                    _ => {}
                }
            }
        }
        meta
    }
}
