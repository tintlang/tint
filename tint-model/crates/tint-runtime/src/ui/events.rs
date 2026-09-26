// ui/events.rs

use super::tree::UiTree;
use crate::vm::TintVM;
use std::collections::HashMap;

pub type EventCallback = String; // name of Tint function

pub struct UiEventSystem {
    pub handlers: HashMap<usize, EventCallback>, // node_id -> callback
}

impl UiEventSystem {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, node_id: usize, fn_name: &str) {
        self.handlers.insert(node_id, fn_name.to_string());
    }

    /// Dispatch events and call Tint functions (placeholder)
    pub fn dispatch(&mut self, _tree: &mut UiTree, _vm: &mut TintVM) {
        // TODO: event loop
        // vm.call_function(fn_name, ...)
    }
}
