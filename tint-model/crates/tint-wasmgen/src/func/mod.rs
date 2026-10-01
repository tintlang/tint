//! Lowering of one typed-IR function to WebAssembly.

use crate::cfg::Cfg;
use crate::{unsupported, val_type, Unsupported};
use std::collections::HashMap;
use tint_ir::typed::*;
use tint_wasmabi::*;
use wasm_encoder::{BlockType, Function, Instruction as I, MemArg, ValType};

// ---- runtime imports ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Imp {
    Trap,
    Fmod,
    CastFail,
    Release,
    ObjNew,
    ObjUnique,
    ImmortalVariant,
    StrLit,
    StrConcat,
    StrLen,
    StrCmp,
    ListNew,
    ListUnique,
    ListPush,
    ListSet,
    ListTake,
    ListPopOpt,
    ListOob,
    MapNew,
    MapUnique,
    MapSet,
    MapGet,
    MapTake,
    MapGetOpt,
    MapLen,
    MapHas,
    Call,
    Cmp,
    ToStr,
    Print,
    RtScratch,
    RtAlloc,
    RtInit,
    UiOpen,
    UiNum,
    UiStr,
    UiBool,
    UiClose,
    UiText,
    UiTokens,
    StorageGetOr,
    StorageSet,
    StorageRemove,
    NowMs,
    HttpGet,
    HostNative,
}

impl Imp {
    pub const COUNT: usize = 47;
}

const I32: ValType = ValType::I32;
const I64: ValType = ValType::I64;
const F64: ValType = ValType::F64;

type Sig = (&'static str, &'static [ValType], &'static [ValType]);

const SIGS: [Sig; Imp::COUNT] = [
    ("trap", &[I32], &[]),
    ("tint_fmod", &[F64, F64], &[F64]),
    ("cast_fail", &[I32, I32, I64], &[]),
    ("retain", &[I32], &[]),
    ("release", &[I32], &[]),
    ("obj_new", &[I32, I64], &[I32]),
    ("obj_unique", &[I32], &[I32]),
    ("immortal_variant", &[I32, I32], &[I32]),
    ("str_lit", &[I32, I32], &[I32]),
    ("str_concat", &[I32, I32], &[I32]),
    ("str_len", &[I32], &[I32]),
    ("str_cmp", &[I32, I32], &[I32]),
    ("list_new", &[I32, I32, I32], &[I32]),
    ("list_unique", &[I32], &[I32]),
    ("list_push", &[I32, I64], &[I32]),
    ("list_set", &[I32, I64, I64], &[I32]),
    ("list_take", &[I32, I64], &[I64]),
    ("list_pop_opt", &[I32, I32, I32, I32, I32], &[I32]),
    ("list_oob", &[I32, I64], &[]),
    ("map_new", &[I32], &[I32]),
    ("map_unique", &[I32], &[I32]),
    ("map_set", &[I32, I32, I64], &[I32]),
    ("map_get", &[I32, I32], &[I64]),
    ("map_take", &[I32, I32], &[I64]),
    ("map_get_opt", &[I32, I32, I32, I32, I32, I32], &[I32]),
    ("map_len", &[I32], &[I32]),
    ("map_has", &[I32, I32], &[I32]),
    ("call", &[I32, I32, I32, I32, I32], &[I64]),
    ("cmp", &[I32, I32, I64, I64], &[I32]),
    ("to_str", &[I32, I64], &[I32]),
    ("print", &[I32, I64, I32], &[]),
    ("rt_scratch", &[I32], &[I32]),
    ("rt_alloc", &[I32], &[I32]),
    ("rt_init", &[I32, I32], &[]),
    ("ui_open", &[I32], &[]),
    ("ui_num", &[F64], &[]),
    ("ui_str", &[I32], &[]),
    ("ui_bool", &[I32], &[]),
    ("ui_close", &[], &[]),
    ("ui_text", &[I32], &[]),
    ("ui_tokens", &[I32], &[]),
    ("storage_get_or", &[I32, I32], &[I32]),
    ("storage_set", &[I32, I32], &[]),
    ("storage_remove", &[I32], &[]),
    ("now_ms", &[], &[F64]),
    ("http_get", &[I32], &[F64]),
    ("host_native", &[I32, I32, I32, I32, I32, I32, I32], &[I64]),
];

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ExtraKey {
    Lit(String),
    Variant(u32, u32),
}

#[derive(Default)]
struct Messages {
    list: Vec<String>,
    ids: HashMap<String, u32>,
}

/// State shared by all functions of the module being built.
pub struct Cx<'a> {
    pub m: &'a Module,
    used: Vec<bool>,
    /// Function index of each runtime import that is imported.
    map: Vec<u32>,
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    import_types: Vec<(&'static str, u32)>,
    msgs: Messages,
    extras: Vec<ExtraKey>,
    extra_ids: HashMap<ExtraKey, u32>,
    index: HashMap<FuncId, u32>,
}

impl<'a> Cx<'a> {
    pub fn new(m: &'a Module, mask: &[bool]) -> Self {
        let mut cx = Cx {
            m,
            used: vec![false; Imp::COUNT],
            map: vec![u32::MAX; Imp::COUNT],
            types: Vec::new(),
            import_types: Vec::new(),
            msgs: Messages::default(),
            extras: Vec::new(),
            extra_ids: HashMap::new(),
            index: HashMap::new(),
        };
        let mut next = 0;
        for (i, (name, params, results)) in SIGS.iter().enumerate() {
            if mask[i] {
                let ty = cx.func_type(params.to_vec(), results.to_vec());
                cx.import_types.push((name, ty));
                cx.map[i] = next;
                next += 1;
            }
        }
        cx
    }

    pub fn nimports(&self) -> u32 {
        self.import_types.len() as u32
    }

    pub fn import_list(&self) -> Vec<(&'static str, u32)> {
        self.import_types.clone()
    }

    pub fn used_imports(&self) -> Vec<bool> {
        self.used.clone()
    }

    pub fn set_indices(&mut self, index: HashMap<FuncId, u32>) {
        self.index = index;
    }

    pub fn func_type(&mut self, params: Vec<ValType>, results: Vec<ValType>) -> u32 {
        if let Some(i) = self
            .types
            .iter()
            .position(|(p, r)| *p == params && *r == results)
        {
            return i as u32;
        }
        self.types.push((params, results));
        (self.types.len() - 1) as u32
    }

    pub fn type_list(&self) -> &[(Vec<ValType>, Vec<ValType>)] {
        &self.types
    }

    fn imp(&mut self, i: Imp) -> u32 {
        self.used[i as usize] = true;
        self.map[i as usize]
    }

    fn msg(&mut self, text: &str) -> i32 {
        if let Some(id) = self.msgs.ids.get(text) {
            return *id as i32;
        }
        let id = self.msgs.list.len() as u32;
        self.msgs.list.push(text.to_owned());
        self.msgs.ids.insert(text.to_owned(), id);
        id as i32
    }

    pub fn messages(&self) -> Vec<String> {
        self.msgs.list.clone()
    }

    fn extra(&mut self, key: ExtraKey) -> u32 {
        if let Some(i) = self.extra_ids.get(&key) {
            return self.m.globals.len() as u32 + *i;
        }
        let i = self.extras.len() as u32;
        self.extras.push(key.clone());
        self.extra_ids.insert(key, i);
        self.m.globals.len() as u32 + i
    }

    pub fn extras(&self) -> &[ExtraKey] {
        &self.extras
    }
}

// ---- helpers ------------------------------------------------------------------------

fn ma(offset: u64, align: u32) -> MemArg {
    MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

fn slot_off(slot: usize) -> u64 {
    OFF_SLOTS + 8 * slot as u64
}

/// Which registers hold owned pointers.
fn heap_regs(m: &Module, f: &Func) -> Vec<bool> {
    f.regs.iter().map(|t| is_heap(m, *t)).collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Frame {
    Loop(usize),
    Block(usize),
    If,
}

mod emit;
mod entry;

pub use emit::compile_func;
pub use entry::{run_callback, start, thunk};
