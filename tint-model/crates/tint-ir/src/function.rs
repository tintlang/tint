use crate::Opcode;

#[derive(Debug, Clone, PartialEq)]
pub enum IRValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Unit,

    // For future use:
    FunctionRef(String), // function name
    Struct(Vec<(String, IRValue)>),
    Array(Vec<IRValue>),
}

#[derive(Debug, Clone)]
pub struct IRFunction {
    pub name: String,

    pub locals: Vec<String>,     // var names → registers/stack slots
    pub constants: Vec<IRValue>, // constant pool
    pub code: Vec<Opcode>,       // emitted bytecode
}

impl IRFunction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            locals: Vec::new(),
            constants: Vec::new(),
            code: Vec::new(),
        }
    }

    /// Add constant to pool, return its index.
    pub fn add_const(&mut self, c: IRValue) -> u32 {
        self.constants.push(c);
        (self.constants.len() - 1) as u32
    }

    /// Create local variable, return its slot index.
    pub fn alloc_local(&mut self, name: impl Into<String>) -> u32 {
        self.locals.push(name.into());
        (self.locals.len() - 1) as u32
    }

    /// Emit bytecode instruction.
    pub fn emit(&mut self, op: Opcode) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }
}
