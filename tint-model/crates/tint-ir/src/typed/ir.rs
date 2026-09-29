//! The typed register IR.
//!
//! A function is a control-flow graph of basic blocks over typed virtual
//! registers. Registers are mutable (this is not SSA): a variable is one
//! register that later instructions overwrite. Every value operation says what
//! it does to which types, so a backend never has to guess: arithmetic names
//! its `NumKind`, aggregates name their ADT, calls to the runtime name an
//! `RtFn`.
//!
//! Values have value semantics. `Mov` copies logically; implementations are
//! free to share storage as long as a write through one register is never
//! visible through another (copy-on-write).

use super::ty::{AdtId, NumKind, TyId, TypeTable};
use super::ui::UiTemplate;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reg(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FuncId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Unit,
    Bool(bool),
    /// An integer of any integer kind. `u64` values are stored as their bit
    /// pattern; the kind of the destination register says how to read it.
    Int(i64),
    /// A float of any float kind (`f32` values are already rounded).
    Float(f64),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// One step into an aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proj {
    /// Field of a struct, by declaration index.
    Field(u32),
    /// Element of a tuple.
    Tuple(u32),
    /// Element of a list; the register holds any numeric index.
    Index(Reg),
    /// Entry of a map; the register holds the key string.
    Key(Reg),
}

/// Calls into the runtime library. Argument and result types follow from the
/// operand registers; `mutates_first` calls update their first argument in
/// place (the register that holds it), everything else leaves arguments alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtFn {
    // strings
    /// Variadic: concatenates any number of strings.
    StrConcat,
    StrLen,
    StrIsEmpty,
    StrTrim,
    StrUpper,
    StrLower,
    StrContains,
    StrStartsWith,
    StrEndsWith,
    /// `(string, start)` or `(string, start, end)`, counted in characters.
    StrSlice,
    StrSplit,
    StrReplace,
    // lists
    ListLen,
    ListIsEmpty,
    ListPush,
    ListPop,
    ListRemove,
    ListReverse,
    ListSort,
    ListSlice,
    ListContains,
    ListJoin,
    // maps
    MapLen,
    MapIsEmpty,
    MapHas,
    MapGet,
    MapSet,
    MapRemove,
    MapKeys,
    MapValues,
    // math on `number`
    Sqrt,
    Min,
    Max,
    Abs,
    Sign,
    Clamp,
    ParseNumber,
    // Vec2 (a struct of two numbers)
    Vec2Length,
    Vec2Normalized,
}

impl RtFn {
    pub fn mutates_first(self) -> bool {
        matches!(
            self,
            RtFn::ListPush | RtFn::ListPop | RtFn::ListRemove | RtFn::MapSet | RtFn::MapRemove
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            RtFn::StrConcat => "str.concat",
            RtFn::StrLen => "str.len",
            RtFn::StrIsEmpty => "str.is_empty",
            RtFn::StrTrim => "str.trim",
            RtFn::StrUpper => "str.to_upper",
            RtFn::StrLower => "str.to_lower",
            RtFn::StrContains => "str.contains",
            RtFn::StrStartsWith => "str.starts_with",
            RtFn::StrEndsWith => "str.ends_with",
            RtFn::StrSlice => "str.slice",
            RtFn::StrSplit => "str.split",
            RtFn::StrReplace => "str.replace",
            RtFn::ListLen => "list.len",
            RtFn::ListIsEmpty => "list.is_empty",
            RtFn::ListPush => "list.push",
            RtFn::ListPop => "list.pop",
            RtFn::ListRemove => "list.remove",
            RtFn::ListReverse => "list.reverse",
            RtFn::ListSort => "list.sort",
            RtFn::ListSlice => "list.slice",
            RtFn::ListContains => "list.contains",
            RtFn::ListJoin => "list.join",
            RtFn::MapLen => "map.len",
            RtFn::MapIsEmpty => "map.is_empty",
            RtFn::MapHas => "map.has",
            RtFn::MapGet => "map.get",
            RtFn::MapSet => "map.set",
            RtFn::MapRemove => "map.remove",
            RtFn::MapKeys => "map.keys",
            RtFn::MapValues => "map.values",
            RtFn::Sqrt => "math.sqrt",
            RtFn::Min => "math.min",
            RtFn::Max => "math.max",
            RtFn::Abs => "math.abs",
            RtFn::Sign => "math.sign",
            RtFn::Clamp => "math.clamp",
            RtFn::ParseNumber => "parse_number",
            RtFn::Vec2Length => "vec2.length",
            RtFn::Vec2Normalized => "vec2.normalized",
        }
    }
}

/// Calls that leave the program: they talk to the host environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostFn {
    /// `print`, `println`, `log`: one debug-formatted line per argument.
    Print,
    /// `dbg`, `debug`.
    Dbg,
    Error,
    ReadLine,
    ReadKey,
}

impl HostFn {
    pub fn name(self) -> &'static str {
        match self {
            HostFn::Print => "print",
            HostFn::Dbg => "dbg",
            HostFn::Error => "error",
            HostFn::ReadLine => "read_line",
            HostFn::ReadKey => "read_key",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    Const { dst: Reg, value: Const },
    Mov { dst: Reg, src: Reg },
    /// Checked arithmetic on two registers of type `Num(kind)`.
    Bin { dst: Reg, op: BinOp, kind: NumKind, a: Reg, b: Reg },
    /// Comparison, `dst: bool`. `Eq`/`Ne` compare any two values of one type
    /// structurally; the ordering operators need numbers or strings.
    Cmp { dst: Reg, op: CmpOp, a: Reg, b: Reg },
    Not { dst: Reg, src: Reg },
    Neg { dst: Reg, src: Reg },
    /// Checked numeric conversion (`as`): fails at run time when the value
    /// does not fit or is not integral.
    Cast { dst: Reg, src: Reg },
    /// The text `"{value}"` produces, for any type.
    ToStr { dst: Reg, src: Reg },
    Tuple { dst: Reg, items: Vec<Reg> },
    /// Fields in declaration order.
    Struct { dst: Reg, adt: AdtId, fields: Vec<Reg> },
    Variant { dst: Reg, adt: AdtId, variant: u32, fields: Vec<Reg> },
    List { dst: Reg, items: Vec<Reg> },
    /// Entries in source order; later duplicates win.
    Map { dst: Reg, entries: Vec<(String, Reg)> },
    /// Reads a part of `base` (shares the storage; use before a mutation only
    /// when the copy is wanted).
    Get { dst: Reg, base: Reg, proj: Proj },
    /// Moves a part out of `base`, leaving it empty until a `Set` fills it
    /// again. Used to modify nested places without copying.
    Take { dst: Reg, base: Reg, proj: Proj },
    /// Overwrites a part of `base`. `Index`/`Key` must exist already, except a
    /// map key, which is inserted.
    Set { base: Reg, proj: Proj, src: Reg },
    /// Index of the variant of an enum value, as `Num(I64)`.
    Tag { dst: Reg, src: Reg },
    /// Field `index` of the payload of `src`, which must be variant `variant`.
    Payload { dst: Reg, src: Reg, variant: u32, index: u32 },
    Rt { dst: Reg, f: RtFn, args: Vec<Reg> },
    Host { dst: Reg, f: HostFn, args: Vec<Reg> },
    Call { dst: Reg, func: FuncId, args: Vec<Reg> },
    CallClosure { dst: Reg, callee: Reg, args: Vec<Reg> },
    /// A function value. The captured registers become the first parameters of
    /// `func` when it is called.
    Closure { dst: Reg, func: FuncId, captures: Vec<Reg> },
    GlobalGet { dst: Reg, global: GlobalId },
    GlobalSet { global: GlobalId, src: Reg },
    /// Starts the element `Module::ui_templates[template]`. `values` fill its
    /// non-`Unused` slots (`UiSlot`), in order.
    UiOpen { template: u32, values: Vec<Reg> },
    /// Finishes the innermost open element.
    UiClose,
    /// A text child (`src: string`) of the innermost open element.
    UiText { src: Reg },
    /// Activates the theme tokens `Module::ui_templates[template]`.
    UiTokens { template: u32 },
}

impl Instr {
    pub fn dst(&self) -> Option<Reg> {
        use Instr::*;
        match self {
            Const { dst, .. }
            | Mov { dst, .. }
            | Bin { dst, .. }
            | Cmp { dst, .. }
            | Not { dst, .. }
            | Neg { dst, .. }
            | Cast { dst, .. }
            | ToStr { dst, .. }
            | Tuple { dst, .. }
            | Struct { dst, .. }
            | Variant { dst, .. }
            | List { dst, .. }
            | Map { dst, .. }
            | Get { dst, .. }
            | Take { dst, .. }
            | Tag { dst, .. }
            | Payload { dst, .. }
            | Rt { dst, .. }
            | Host { dst, .. }
            | Call { dst, .. }
            | CallClosure { dst, .. }
            | Closure { dst, .. }
            | GlobalGet { dst, .. } => Some(*dst),
            Set { .. } | GlobalSet { .. } | UiOpen { .. } | UiClose | UiText { .. } | UiTokens { .. } => None,
        }
    }

    /// Every register the instruction reads (or updates in place).
    pub fn uses(&self) -> Vec<Reg> {
        use Instr::*;
        let proj = |p: &Proj, out: &mut Vec<Reg>| match p {
            Proj::Index(r) | Proj::Key(r) => out.push(*r),
            _ => {}
        };
        let mut out = Vec::new();
        match self {
            Const { .. } | GlobalGet { .. } | UiClose | UiTokens { .. } => {}
            Mov { src, .. }
            | Not { src, .. }
            | Neg { src, .. }
            | Cast { src, .. }
            | ToStr { src, .. }
            | Tag { src, .. }
            | Payload { src, .. }
            | GlobalSet { src, .. }
            | UiText { src } => out.push(*src),
            Bin { a, b, .. } | Cmp { a, b, .. } => {
                out.push(*a);
                out.push(*b);
            }
            Tuple { items, .. } | List { items, .. } => out.extend(items),
            Struct { fields, .. } | Variant { fields, .. } => out.extend(fields),
            Map { entries, .. } => out.extend(entries.iter().map(|(_, r)| *r)),
            Get { base, proj: p, .. } | Take { base, proj: p, .. } => {
                out.push(*base);
                proj(p, &mut out);
            }
            Set { base, proj: p, src } => {
                out.push(*base);
                proj(p, &mut out);
                out.push(*src);
            }
            Rt { args, .. } | Host { args, .. } | Call { args, .. } => out.extend(args),
            UiOpen { values, .. } => out.extend(values),
            CallClosure { callee, args, .. } => {
                out.push(*callee);
                out.extend(args);
            }
            Closure { captures, .. } => out.extend(captures),
        }
        out
    }

    /// Registers whose value the instruction replaces without reading it
    /// first, in addition to `dst`: the base of a `Set`, the first argument of
    /// a mutating runtime call.
    pub fn updates(&self) -> Option<Reg> {
        match self {
            Instr::Set { base, .. } | Instr::Take { base, .. } => Some(*base),
            Instr::Rt { f, args, .. } if f.mutates_first() => args.first().copied(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    Jump(BlockId),
    Branch { cond: Reg, then_: BlockId, else_: BlockId },
    /// Jumps on the integer value of `value` (a `Tag`).
    Switch { value: Reg, cases: Vec<(i64, BlockId)>, default: BlockId },
    Return(Reg),
    /// Stops the program with an error.
    Trap(String),
}

impl Term {
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Term::Jump(b) => vec![*b],
            Term::Branch { then_, else_, .. } => vec![*then_, *else_],
            Term::Switch { cases, default, .. } => {
                let mut out: Vec<BlockId> = cases.iter().map(|(_, b)| *b).collect();
                out.push(*default);
                out
            }
            Term::Return(_) | Term::Trap(_) => vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub instrs: Vec<Instr>,
    pub term: Term,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncKind {
    Fn,
    /// A method. With `inout_self` the function returns `(result, self)` and
    /// the caller writes the second part back into the receiver.
    Method { inout_self: bool },
    Lambda,
    /// Evaluates the initializers of all globals.
    Init,
    /// A `ui fn`: emits a tree (see `ui`) and returns unit.
    Ui,
}

#[derive(Debug, Clone)]
pub struct Func {
    pub name: String,
    pub kind: FuncKind,
    /// Captured values first, then the declared parameters.
    pub params: Vec<Reg>,
    pub ncaptures: u32,
    pub ret: TyId,
    /// Type of every register.
    pub regs: Vec<TyId>,
    /// Block 0 is the entry.
    pub blocks: Vec<Block>,
}

impl Func {
    pub fn reg_ty(&self, reg: Reg) -> TyId {
        self.regs[reg.0 as usize]
    }
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub ty: TyId,
}

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub types: TypeTable,
    pub funcs: Vec<Func>,
    pub globals: Vec<Global>,
    /// Runs the initializers of all globals; call it once before anything else.
    pub init: Option<FuncId>,
    /// Top-level functions by name.
    pub functions: HashMap<String, FuncId>,
    /// User methods by (type, name).
    pub methods: HashMap<(String, String), FuncId>,
    /// Static parts of the UI elements the `Ui` functions emit.
    pub ui_templates: Vec<UiTemplate>,
}

impl Module {
    pub fn func(&self, id: FuncId) -> &Func {
        &self.funcs[id.0 as usize]
    }
}
