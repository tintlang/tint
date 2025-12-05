#[derive(Debug, Clone)]
pub enum RuntimeValue {
    // primitive scalars
    Number(f64),
    Bool(bool),
    String(String),
    Unit,

    // high-level runtime values
    List(Vec<RuntimeValue>),
    Tuple(Vec<RuntimeValue>),

    StructInstance {
        name: String,
        fields: Vec<(String, RuntimeValue)>,
    },

    EnumInstance {
        enum_name: String,
        variant: String,
        args: Vec<RuntimeValue>,
    },

    Map(std::collections::HashMap<String, RuntimeValue>),

    // low-level VM handles
    Struct(usize),
    Enum(usize, usize),
    Function(usize),
    UiNode(usize),
    Resource(usize),
    Future(usize),
}
