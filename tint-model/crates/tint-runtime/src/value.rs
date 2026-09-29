#[derive(Debug, Clone)]
pub enum RuntimeValue {
    // primitive scalars
    Number(f64),
    I32(i32),
    I64(i64),
    U8(u8),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
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

    Lambda {
        params: Vec<String>,
        body: Box<tint_ast::Expr>,
        closure: std::rc::Rc<tint_evaluator::Env>,
    },

    // low-level VM handles
    Struct(usize),
    Enum(usize, usize),
    Function(usize),
    FunctionValue {
        params: Vec<String>,
        body: tint_evaluator::eval_fn::FnBodyKind,
        env: std::rc::Rc<tint_evaluator::Env>,
    },
    UiNode(usize),
    Resource(usize),
    Future(usize),
}
