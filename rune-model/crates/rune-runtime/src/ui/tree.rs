// ui/tree.rs

pub type UiNodeId = usize;

#[derive(Debug, Clone)]
pub struct UiElement {
    pub id: UiNodeId,
    pub tag: String,
    pub children: Vec<UiNodeId>,
}

impl UiElement {
    pub fn new(id: UiNodeId, tag: String) -> Self {
        Self {
            id,
            tag,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiTree {
    pub nodes: Vec<UiElement>,
}

impl UiTree {
    pub fn empty() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn create_node(&mut self, tag: String) -> UiNodeId {
        let id = self.nodes.len();
        self.nodes.push(UiElement::new(id, tag));
        id
    }

    pub fn add_child(&mut self, parent: UiNodeId, child: UiNodeId) {
        if let Some(p) = self.nodes.get_mut(parent) {
            p.children.push(child);
        }
    }
}
