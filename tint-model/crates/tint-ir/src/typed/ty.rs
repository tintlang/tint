//! Types of the typed IR.
//!
//! Every register has exactly one `TyId`. Types are interned, so equality of
//! ids is equality of types. Generic structs and enums (including `Option` and
//! `Result`) are monomorphized: `Option<i32>` and `Option<string>` are two
//! distinct ADTs with their own field types.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdtId(pub u32);

/// The numeric types. `Num` is the type of unsuffixed numbers; it computes
/// like `f64` and differs only in how it is written (`3`, not `3f64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NumKind {
    Num,
    I32,
    I64,
    U8,
    U32,
    U64,
    F32,
    F64,
}

impl NumKind {
    pub fn is_float(self) -> bool {
        matches!(self, NumKind::Num | NumKind::F32 | NumKind::F64)
    }

    pub fn is_int(self) -> bool {
        !self.is_float()
    }

    pub fn is_signed(self) -> bool {
        matches!(self, NumKind::I32 | NumKind::I64)
    }

    /// Source spelling; `Num` is `number`.
    pub fn name(self) -> &'static str {
        match self {
            NumKind::Num => "number",
            NumKind::I32 => "i32",
            NumKind::I64 => "i64",
            NumKind::U8 => "u8",
            NumKind::U32 => "u32",
            NumKind::U64 => "u64",
            NumKind::F32 => "f32",
            NumKind::F64 => "f64",
        }
    }

    pub fn from_name(name: &str) -> Option<NumKind> {
        Some(match name {
            "number" => NumKind::Num,
            "i32" => NumKind::I32,
            "i64" => NumKind::I64,
            "u8" => NumKind::U8,
            "u32" => NumKind::U32,
            "u64" => NumKind::U64,
            "f32" => NumKind::F32,
            "f64" => NumKind::F64,
            _ => return None,
        })
    }

    /// Inclusive value range of an integer kind.
    pub fn int_range(self) -> (i128, i128) {
        match self {
            NumKind::I32 => (i32::MIN as i128, i32::MAX as i128),
            NumKind::I64 => (i64::MIN as i128, i64::MAX as i128),
            NumKind::U8 => (0, u8::MAX as i128),
            NumKind::U32 => (0, u32::MAX as i128),
            NumKind::U64 => (0, u64::MAX as i128),
            _ => (0, 0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TyKind {
    Unit,
    Bool,
    Num(NumKind),
    Str,
    List(TyId),
    /// Keys are always strings.
    Map(TyId),
    Tuple(Vec<TyId>),
    Adt(AdtId),
    /// A function value: a closure (code plus captured values).
    Fn(Vec<TyId>, TyId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: TyId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    pub name: String,
    /// Payload fields. Positional payloads are named `0`, `1`, ...
    pub fields: Vec<FieldDef>,
    pub positional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AdtBody {
    Struct(Vec<FieldDef>),
    Enum(Vec<VariantDef>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdtDef {
    /// Declared name without type arguments (`Option`, `Point`).
    pub base: String,
    pub args: Vec<TyId>,
    pub body: AdtBody,
}

impl AdtDef {
    pub fn is_enum(&self) -> bool {
        matches!(self.body, AdtBody::Enum(_))
    }

    pub fn struct_fields(&self) -> &[FieldDef] {
        match &self.body {
            AdtBody::Struct(fields) => fields,
            AdtBody::Enum(_) => &[],
        }
    }

    pub fn variants(&self) -> &[VariantDef] {
        match &self.body {
            AdtBody::Enum(variants) => variants,
            AdtBody::Struct(_) => &[],
        }
    }

    pub fn variant_index(&self, name: &str) -> Option<u32> {
        self.variants().iter().position(|v| v.name == name).map(|i| i as u32)
    }

    pub fn field_index(&self, name: &str) -> Option<u32> {
        self.struct_fields().iter().position(|f| f.name == name).map(|i| i as u32)
    }
}

/// Interner for types and the table of monomorphized ADTs.
#[derive(Debug, Clone, Default)]
pub struct TypeTable {
    kinds: Vec<TyKind>,
    index: HashMap<TyKind, TyId>,
    adts: Vec<AdtDef>,
    adt_index: HashMap<(String, Vec<TyId>), AdtId>,
}

impl TypeTable {
    pub fn new() -> Self {
        TypeTable::default()
    }

    pub fn intern(&mut self, kind: TyKind) -> TyId {
        if let Some(id) = self.index.get(&kind) {
            return *id;
        }
        let id = TyId(self.kinds.len() as u32);
        self.kinds.push(kind.clone());
        self.index.insert(kind, id);
        id
    }

    pub fn kind(&self, id: TyId) -> &TyKind {
        &self.kinds[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn unit(&mut self) -> TyId {
        self.intern(TyKind::Unit)
    }

    pub fn bool(&mut self) -> TyId {
        self.intern(TyKind::Bool)
    }

    pub fn str(&mut self) -> TyId {
        self.intern(TyKind::Str)
    }

    pub fn num(&mut self, kind: NumKind) -> TyId {
        self.intern(TyKind::Num(kind))
    }

    pub fn list(&mut self, item: TyId) -> TyId {
        self.intern(TyKind::List(item))
    }

    pub fn map(&mut self, item: TyId) -> TyId {
        self.intern(TyKind::Map(item))
    }

    pub fn tuple(&mut self, items: Vec<TyId>) -> TyId {
        self.intern(TyKind::Tuple(items))
    }

    pub fn func(&mut self, params: Vec<TyId>, ret: TyId) -> TyId {
        self.intern(TyKind::Fn(params, ret))
    }

    pub fn adt_ty(&mut self, id: AdtId) -> TyId {
        self.intern(TyKind::Adt(id))
    }

    /// Reserves an ADT so that its fields may refer to it (recursive types).
    /// Returns `(id, true)` when the ADT is new and its body still has to be
    /// filled in with `set_adt_body`.
    pub fn declare_adt(&mut self, base: &str, args: &[TyId]) -> (AdtId, bool) {
        let key = (base.to_string(), args.to_vec());
        if let Some(id) = self.adt_index.get(&key) {
            return (*id, false);
        }
        let id = AdtId(self.adts.len() as u32);
        self.adts.push(AdtDef {
            base: base.to_string(),
            args: args.to_vec(),
            body: AdtBody::Struct(Vec::new()),
        });
        self.adt_index.insert(key, id);
        (id, true)
    }

    pub fn set_adt_body(&mut self, id: AdtId, body: AdtBody) {
        self.adts[id.0 as usize].body = body;
    }

    pub fn adt(&self, id: AdtId) -> &AdtDef {
        &self.adts[id.0 as usize]
    }

    pub fn adts(&self) -> &[AdtDef] {
        &self.adts
    }

    pub fn as_adt(&self, ty: TyId) -> Option<AdtId> {
        match self.kind(ty) {
            TyKind::Adt(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_num(&self, ty: TyId) -> Option<NumKind> {
        match self.kind(ty) {
            TyKind::Num(kind) => Some(*kind),
            _ => None,
        }
    }

    /// Source-like spelling, for diagnostics and dumps.
    pub fn show(&self, ty: TyId) -> String {
        match self.kind(ty) {
            TyKind::Unit => "unit".into(),
            TyKind::Bool => "bool".into(),
            TyKind::Num(kind) => kind.name().into(),
            TyKind::Str => "string".into(),
            TyKind::List(item) => format!("[{}]", self.show(*item)),
            TyKind::Map(item) => format!("map<{}>", self.show(*item)),
            TyKind::Tuple(items) => {
                let items: Vec<String> = items.iter().map(|t| self.show(*t)).collect();
                format!("({})", items.join(", "))
            }
            TyKind::Adt(id) => self.show_adt(*id),
            TyKind::Fn(params, ret) => {
                let params: Vec<String> = params.iter().map(|t| self.show(*t)).collect();
                format!("fn({}) -> {}", params.join(", "), self.show(*ret))
            }
        }
    }

    pub fn show_adt(&self, id: AdtId) -> String {
        let def = self.adt(id);
        if def.args.is_empty() {
            def.base.clone()
        } else {
            let args: Vec<String> = def.args.iter().map(|t| self.show(*t)).collect();
            format!("{}<{}>", def.base, args.join(", "))
        }
    }
}
