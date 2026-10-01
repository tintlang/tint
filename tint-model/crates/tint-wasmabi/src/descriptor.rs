use std::collections::HashMap;

use tint_ir::typed::*;

use crate::{kind_index, KINDS};

const MAGIC: &[u8; 4] = b"TNT1";

struct W(Vec<u8>);

impl W {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn tys(&mut self, tys: &[TyId]) {
        self.u32(tys.len() as u32);
        for t in tys {
            self.u32(t.0);
        }
    }
}

/// The part of a module the runtime needs: the type table (to convert between
/// objects in memory and interpreter values), what closures capture, and the
/// text of the traps.
pub fn encode(m: &Module, msgs: &[String], meta: &str, sigs: &[(String, String)]) -> Vec<u8> {
    let mut w = W(MAGIC.to_vec());
    w.u32(m.types.len() as u32);
    for i in 0..m.types.len() {
        match m.types.kind(TyId(i as u32)) {
            TyKind::Unit => w.u32(0),
            TyKind::Bool => w.u32(1),
            TyKind::Num(k) => {
                w.u32(2);
                w.u32(kind_index(*k));
            }
            TyKind::Str => w.u32(3),
            TyKind::List(t) => {
                w.u32(4);
                w.u32(t.0);
            }
            TyKind::Map(t) => {
                w.u32(5);
                w.u32(t.0);
            }
            TyKind::Tuple(ts) => {
                w.u32(6);
                w.tys(ts);
            }
            TyKind::Adt(a) => {
                w.u32(7);
                w.u32(a.0);
            }
            TyKind::Fn(ps, r) => {
                w.u32(8);
                w.tys(ps);
                w.u32(r.0);
            }
        }
    }
    w.u32(m.types.adts().len() as u32);
    for def in m.types.adts() {
        w.str(&def.base);
        w.tys(&def.args);
        match &def.body {
            AdtBody::Struct(fields) => {
                w.u32(0);
                w.u32(fields.len() as u32);
                for f in fields {
                    w.str(&f.name);
                    w.u32(f.ty.0);
                }
            }
            AdtBody::Enum(variants) => {
                w.u32(1);
                w.u32(variants.len() as u32);
                for v in variants {
                    w.str(&v.name);
                    w.u32(v.positional as u32);
                    w.u32(v.fields.len() as u32);
                    for f in &v.fields {
                        w.str(&f.name);
                        w.u32(f.ty.0);
                    }
                }
            }
        }
    }
    w.u32(m.funcs.len() as u32);
    for f in &m.funcs {
        w.u32(f.ncaptures);
        let tys: Vec<TyId> = f.params.iter().map(|p| f.reg_ty(*p)).collect();
        w.tys(&tys);
        w.u32(f.ret.0);
    }
    w.u32(msgs.len() as u32);
    for s in msgs {
        w.str(s);
    }
    w.str(&serde_json::to_string(&m.ui_templates).expect("templates serialize"));
    w.str(meta);
    w.u32(m.natives.len() as u32);
    for n in &m.natives {
        w.str(n);
    }
    w.u32(sigs.len() as u32);
    for (name, sig) in sigs {
        w.str(name);
        w.str(sig);
    }
    w.0
}

struct R<'a>(&'a [u8], usize);

impl R<'_> {
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.0[self.1..self.1 + 4].try_into().unwrap());
        self.1 += 4;
        v
    }
    fn str(&mut self) -> String {
        let n = self.u32() as usize;
        let s = String::from_utf8_lossy(&self.0[self.1..self.1 + n]).into_owned();
        self.1 += n;
        s
    }
    fn tys(&mut self) -> Vec<TyId> {
        let n = self.u32();
        (0..n).map(|_| TyId(self.u32())).collect()
    }
}

/// Inverse of [`encode`]: a module with the same types and function
/// signatures (the functions have no code) and the trap messages.
pub fn decode(bytes: &[u8]) -> (Module, Vec<String>, String) {
    assert_eq!(&bytes[..4], MAGIC, "bad module descriptor");
    let mut r = R(bytes, 4);
    let mut m = Module::default();
    let n = r.u32();
    for _ in 0..n {
        let kind = match r.u32() {
            0 => TyKind::Unit,
            1 => TyKind::Bool,
            2 => TyKind::Num(KINDS[r.u32() as usize]),
            3 => TyKind::Str,
            4 => TyKind::List(TyId(r.u32())),
            5 => TyKind::Map(TyId(r.u32())),
            6 => TyKind::Tuple(r.tys()),
            7 => TyKind::Adt(AdtId(r.u32())),
            8 => {
                let ps = r.tys();
                TyKind::Fn(ps, TyId(r.u32()))
            }
            t => panic!("bad type tag {t}"),
        };
        m.types.intern(kind);
    }
    let n = r.u32();
    for _ in 0..n {
        let base = r.str();
        let args = r.tys();
        let (id, _) = m.types.declare_adt(&base, &args);
        let body = if r.u32() == 0 {
            let nf = r.u32();
            AdtBody::Struct(
                (0..nf)
                    .map(|_| FieldDef {
                        name: r.str(),
                        ty: TyId(r.u32()),
                    })
                    .collect(),
            )
        } else {
            let nv = r.u32();
            AdtBody::Enum(
                (0..nv)
                    .map(|_| {
                        let name = r.str();
                        let positional = r.u32() != 0;
                        let nf = r.u32();
                        let fields = (0..nf)
                            .map(|_| FieldDef {
                                name: r.str(),
                                ty: TyId(r.u32()),
                            })
                            .collect();
                        VariantDef {
                            name,
                            fields,
                            positional,
                        }
                    })
                    .collect(),
            )
        };
        m.types.set_adt_body(id, body);
    }
    let n = r.u32();
    for _ in 0..n {
        let ncaptures = r.u32();
        let tys = r.tys();
        let ret = TyId(r.u32());
        m.funcs.push(Func {
            name: "f".into(),
            kind: FuncKind::Fn,
            params: (0..tys.len() as u32).map(Reg).collect(),
            ncaptures,
            ret,
            regs: tys,
            blocks: Vec::new(),
        });
    }
    let n = r.u32();
    let msgs = (0..n).map(|_| r.str()).collect();
    let templates = r.str();
    m.ui_templates = serde_json::from_str(&templates).expect("templates deserialize");
    let meta = r.str();
    let n = r.u32();
    m.natives = (0..n).map(|_| r.str()).collect();
    let n = r.u32();
    m.export_sigs = (0..n).map(|_| (r.str(), r.str())).collect();
    m.functions = HashMap::new();
    (m, msgs, meta)
}
