use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Unit,
    Tuple(Vec<Value>),
    List(Vec<Value>),
    StructInstance {
        name: String,
        fields: Vec<(String, Value)>,
    },
    EnumInstance {
        enum_name: String,
        variant: String,
        fields: Vec<(String, Value)>,
    },
    Map(HashMap<String, Value>),
}

impl Value {
    pub fn unwrap_number(&self) -> f64 {
        match self {
            Self::Number(value) => *value,
            other => panic!("Expected number, got {:?}", other),
        }
    }

    pub fn unwrap_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            other => panic!("Expected bool, got {:?}", other),
        }
    }

    pub fn unwrap_string(&self) -> &str {
        match self {
            Self::String(value) => value,
            other => panic!("Expected string, got {:?}", other),
        }
    }
}
