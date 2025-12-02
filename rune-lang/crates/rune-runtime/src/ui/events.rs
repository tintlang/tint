// ui/events.rs

use std::collections::HashMap;

pub type EventCallback = usize;

pub struct UiEventSystem {
    pub handlers: HashMap<String, EventCallback>,
}

impl UiEventSystem {
    pub fn new() -> Self {
        Self { handlers: HashMap::new() }
    }

    pub fn register(&mut self, name: &str, id: usize) {
        self.handlers.insert(name.to_string(), id);
    }
}
