use std::collections::HashSet;

#[derive(Default, Debug)]
pub struct Symbols {
    pub types: HashSet<String>,
    pub functions: HashSet<String>,
}

impl Symbols {
    pub fn is_type(&self, name: &str) -> bool {
        self.types.contains(name)
    }

    pub fn is_function(&self, name: &str) -> bool {
        self.functions.contains(name)
    }
}
