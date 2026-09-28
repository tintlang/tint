#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Attribute {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttributeList {
    pub items: Vec<Attribute>,
}

impl AttributeList {
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    pub fn extend(&mut self, other: AttributeList) {
        self.items.extend(other.items);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
