use super::types::Resource;

pub struct ResourceTable {
    items: Vec<Resource>,
}

impl ResourceTable {
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    pub fn insert(&mut self, res: Resource) -> usize {
        let id = self.items.len();
        self.items.push(res);
        id
    }
}
