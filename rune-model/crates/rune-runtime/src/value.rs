#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Number(f64),
    Bool(bool),
    String(String),

    Struct(usize),
    Enum(usize, usize),

    Function(usize),

    UiNode(usize),

    Resource(usize),

    Future(usize),

    Unit,
}
