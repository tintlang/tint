// ui/tree.rs

pub type UiNodeId = usize;

#[derive(Debug)]
pub struct UiTree {
    pub nodes: Vec<UiElement>,
}

#[derive(Debug)]
pub struct UiElement {
    pub id: UiNodeId,
    pub tag: String,
    pub children: Vec<UiNodeId>,
}
