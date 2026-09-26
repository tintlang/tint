use crate::type_table::{Type, TypeTable};
use std::collections::HashSet;

pub struct Scope {
    pub types: TypeTable,
    pub idents: HashSet<String>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            types: TypeTable::default(),
            idents: HashSet::new(),
        }
    }

    pub fn define(&mut self, name: &str, ty: Type) -> bool {
        if self.idents.contains(name) {
            return false;
        }
        self.idents.insert(name.to_string());
        self.types.set(name, ty);
        true
    }

    pub fn lookup(&self, name: &str) -> Option<Type> {
        self.types.get(name)
    }
}

// Stack of scopes
pub struct ScopeStack {
    stack: Vec<Scope>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            stack: vec![Scope::new()],
        }
    }

    pub fn push(&mut self) {
        self.stack.push(Scope::new());
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }

    pub fn define(&mut self, name: &str, ty: Type) -> bool {
        self.stack.last_mut().unwrap().define(name, ty)
    }

    pub fn lookup(&self, name: &str) -> Option<Type> {
        for scope in self.stack.iter().rev() {
            if let Some(t) = scope.lookup(name) {
                return Some(t);
            }
        }
        None
    }
}
