//! UI in the typed IR.
//!
//! A `ui fn` lowers to an ordinary function of kind `FuncKind::Ui` that
//! *emits* the tree it describes: `UiOpen` starts an element, `UiText` adds a
//! text child, `UiClose` finishes the innermost element. Everything that
//! depends on run-time values (`if{}`, `for{}`, `match{}`, interpolations,
//! modifier expressions) is normal IR around those instructions; everything
//! that does not (styles, components, slots, event handler names, literal
//! modifiers) is resolved while lowering and stored in a `UiTemplate`.
//!
//! What builds a tree out of the emitted stream is the `UiHost` the
//! interpreter (or another backend) runs the function with.

use tint_ast::{Expr, UiAttribute, UiModifier, UiModifierValue};

/// Which kind of value fills one expression of an element's modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UiSlot {
    Number,
    Str,
    Bool,
    /// The expression exists in the modifiers but produces no value at run
    /// time (a `for{}` iterable, or a value that is not a scalar); a host
    /// treats it as unset.
    Unused,
}

/// The static part of an element.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiElementTemplate {
    pub tag: String,
    /// Modifiers after `use::Style` expansion and component merging, exactly
    /// as they are written, with their expressions still in place.
    pub modifiers: Vec<UiModifier>,
    pub attributes: Vec<UiAttribute>,
    /// One per expression of `modifiers`, in the order of `modifier_exprs`.
    pub slots: Vec<UiSlot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UiTemplate {
    Element(UiElementTemplate),
    /// The `tokens { .. }` blocks of a theme that turned out to be active.
    Tokens { modifiers: Vec<UiModifier> },
}

/// A run-time value of a modifier expression.
#[derive(Debug, Clone, PartialEq)]
pub enum UiValue {
    Number(f64),
    Str(String),
    Bool(bool),
}

/// What runs a `Ui` function: it receives the emitted tree.
pub trait UiHost {
    /// Starts an element. `values` has one entry per slot of the template
    /// that is not `UiSlot::Unused`, in order.
    fn open(&mut self, template: &UiElementTemplate, values: &[UiValue]);
    /// Finishes the innermost open element and adds it to its parent.
    fn close(&mut self);
    /// Adds a text child to the innermost open element.
    fn text(&mut self, text: &str);
    /// Makes the tokens of an active theme available to later elements.
    fn tokens(&mut self, modifiers: &[UiModifier]);
}

/// Every expression inside `modifiers`, in the order a builder evaluates
/// them: modifiers in sequence, and inside a value, blocks and tuples in
/// sequence.
pub fn modifier_exprs(modifiers: &[UiModifier]) -> Vec<&Expr> {
    modifier_exprs_of(modifiers)
}

/// One emitted UI instruction, as an interpreter records it.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    Open { template: u32, values: Vec<UiValue> },
    Close,
    Text(String),
    Tokens { template: u32 },
}

/// Feeds recorded events to a host.
pub fn replay(templates: &[UiTemplate], events: &[UiEvent], host: &mut dyn UiHost) {
    for event in events {
        match event {
            UiEvent::Open { template, values } => {
                if let Some(UiTemplate::Element(t)) = templates.get(*template as usize) {
                    host.open(t, values);
                }
            }
            UiEvent::Close => host.close(),
            UiEvent::Text(s) => host.text(s),
            UiEvent::Tokens { template } => {
                if let Some(UiTemplate::Tokens { modifiers }) = templates.get(*template as usize) {
                    host.tokens(modifiers);
                }
            }
        }
    }
}

/// Attributes whose value is a run-time expression (`placeholder||{hint}`, `props||{..}`);
/// their expressions follow the modifiers' in a template's slots.
pub const VALUE_ATTRS: [&str; 7] = ["placeholder", "disabled", "readonly", "title", "alt", "tabindex", "props"];

pub fn attr_exprs(attributes: &[UiAttribute]) -> Vec<&Expr> {
    attributes
        .iter()
        .filter(|a| VALUE_ATTRS.contains(&a.name.as_str()))
        .filter_map(|a| match &a.value {
            tint_ast::UiAttrValue::Expr(e) => Some(e),
            _ => None,
        })
        .collect()
}

/// `modifier_exprs` over modifiers held by reference.
pub fn modifier_exprs_of<'a>(modifiers: impl IntoIterator<Item = &'a UiModifier>) -> Vec<&'a Expr> {
    fn value<'a>(v: &'a UiModifierValue, out: &mut Vec<&'a Expr>) {
        match v {
            UiModifierValue::Expr(e) => out.push(e),
            UiModifierValue::Block(items) => {
                for item in items {
                    value(&item.value, out);
                }
            }
            UiModifierValue::Tuple(items) => {
                for item in items {
                    value(item, out);
                }
            }
            UiModifierValue::MiniMod { value: inner, .. } => value(inner, out),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for m in modifiers {
        value(&m.value, &mut out);
    }
    out
}

/// Names of the functions the templates use as event handlers
/// (`click||name`, `frame||name`, ...), in first-use order.
pub fn ui_handlers(templates: &[UiTemplate], functions: &std::collections::HashMap<String, super::FuncId>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for template in templates {
        let UiTemplate::Element(e) = template else { continue };
        for attr in &e.attributes {
            if let tint_ast::UiAttrValue::Ident(name) = &attr.value {
                if functions.contains_key(name) && !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
    }
    out
}
