use crate::{clif_ty, elem_layout, is_heap, is_scalar, ptr_mask, rt, unsupported, Unsupported};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, FuncRef, InstBuilder, MemFlagsData, Signature, StackSlotData, StackSlotKind,
    TrapCode, Type, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use std::collections::HashMap;
use tint_ir::typed::*;

pub(crate) type Imports = HashMap<&'static str, FuncRef>;

trait ImportExt {
    fn f(&self, name: &str) -> FuncRef;
}

impl ImportExt for Imports {
    fn f(&self, name: &str) -> FuncRef {
        self[name]
    }
}

fn slot_off(slot: usize) -> i32 {
    rt::OFF_SLOTS + 8 * slot as i32
}

fn flags() -> MemFlagsData {
    MemFlagsData::new().with_notrap()
}

/// Scalar register value -> 64-bit slot bits.
fn to_bits(b: &mut FunctionBuilder, v: Value) -> Value {
    match b.func.dfg.value_type(v) {
        t if t == types::F64 => b.ins().bitcast(types::I64, MemFlagsData::new(), v),
        t if t == types::I8 => b.ins().uextend(types::I64, v),
        _ => v,
    }
}

/// 64-bit slot bits -> register value of clif type `t`.
fn from_bits(b: &mut FunctionBuilder, bits: Value, t: Type) -> Value {
    if t == types::F64 {
        b.ins().bitcast(types::F64, MemFlagsData::new(), bits)
    } else if t == types::I8 {
        b.ins().ireduce(types::I8, bits)
    } else {
        bits
    }
}

/// The entry point closures are called through: `(closure, declared params...)`.
/// It unpacks the captured values from the closure object and calls the real
/// function, which takes them as its first parameters.
pub(crate) fn thunk_body(
    m: &Module,
    f: &Func,
    b: &mut FunctionBuilder,
    real: FuncRef,
    retain: FuncRef,
) {
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    b.switch_to_block(entry);
    let params = b.block_params(entry).to_vec();
    let closure = params[0];
    let mut args = Vec::new();
    for i in 0..f.ncaptures as usize {
        let ty = f.reg_ty(f.params[i]);
        let bits = b.ins().load(types::I64, flags(), closure, slot_off(1 + i));
        if is_heap(m, ty) {
            // The callee owns its parameters; the closure keeps its own reference.
            b.ins().call(retain, &[bits]);
            args.push(bits);
        } else {
            let t = clif_ty(m, ty).unwrap_or(types::I64);
            args.push(from_bits(b, bits, t));
        }
    }
    args.extend_from_slice(&params[1..]);
    let call = b.ins().call(real, &args);
    let r = b.inst_results(call)[0];
    b.ins().return_(&[r]);
}

struct L<'a, 'b> {
    m: &'a Module,
    f: &'a Func,
    b: &'a mut FunctionBuilder<'b>,
    imp: &'a Imports,
    callees: &'a [Option<FuncRef>],
    vars: Vec<Variable>,
    heap: Vec<bool>,
    msgs: &'a mut Vec<String>,
    /// Address of the table of closure entry points, indexed by function id.
    fn_table: i64,
    /// Address of the slots of the global variables.
    globals: i64,
    /// Heap registers the current instruction reads for the last time.
    dying: Vec<Reg>,
    /// Of those, the ones whose reference the instruction took over.
    moved: Vec<Reg>,
}

pub(crate) fn lower_func(
    m: &Module,
    f: &Func,
    b: &mut FunctionBuilder,
    imp: &Imports,
    callees: &[Option<FuncRef>],
    msgs: &mut Vec<String>,
    fn_table: i64,
    globals: i64,
) -> Result<(), Unsupported> {
    let mut vars = Vec::new();
    for ty in &f.regs {
        let t = clif_ty(m, *ty).unwrap_or(types::I64);
        vars.push(b.declare_var(t));
    }
    // Reject the function up front if any register has an unsupported type.
    for ty in &f.regs {
        clif_ty(m, *ty)?;
    }
    let heap: Vec<bool> = f.regs.iter().map(|t| is_heap(m, *t)).collect();
    let blocks: Vec<_> = f.blocks.iter().map(|_| b.create_block()).collect();
    b.append_block_params_for_function_params(blocks[0]);
    b.switch_to_block(blocks[0]);
    for (i, h) in heap.iter().enumerate() {
        if *h && !f.params.iter().any(|p| p.0 as usize == i) {
            let null = b.ins().iconst(types::I64, 0);
            b.def_var(vars[i], null);
        }
    }
    for (i, p) in f.params.iter().enumerate() {
        let v = b.block_params(blocks[0])[i];
        b.def_var(vars[p.0 as usize], v);
    }
    let dies = dying_regs(f, &heap);
    let mut l = L {
        m,
        f,
        b,
        imp,
        callees,
        vars,
        heap,
        msgs,
        fn_table,
        globals,
        dying: Vec::new(),
        moved: Vec::new(),
    };

    for (bi, block) in f.blocks.iter().enumerate() {
        if bi != 0 {
            l.b.switch_to_block(blocks[bi]);
        }
        let mut skip_next = false;
        let mut skip_more = 0usize;
        for (ii, ins) in block.instrs.iter().enumerate() {
            if skip_more > 0 {
                skip_more -= 1;
                continue;
            }
            if std::mem::take(&mut skip_next) {
                continue;
            }
            l.dying = dies[bi][ii].clone();
            l.moved.clear();
            // `x = list[i]; y = x.k` with `x` dead afterwards and `y` a scalar:
            // read the field in place, without taking and giving back a reference.
            let fused = match (ins, block.instrs.get(ii + 1)) {
                (
                    Instr::Get {
                        dst: t,
                        base,
                        proj: Proj::Index(i),
                    },
                    Some(Instr::Get {
                        dst: d,
                        base: b2,
                        proj: Proj::Field(k) | Proj::Tuple(k),
                    }),
                ) if b2 == t
                    && t != d
                    && l.heap[t.0 as usize]
                    && !l.heap[d.0 as usize]
                    && dies[bi][ii + 1].contains(t)
                    && !dies[bi][ii].contains(t) =>
                {
                    Some((*base, *i, *d, *k))
                }
                _ => None,
            };
            // `t = take list[i]; t.k = v; list[i] = t` for a scalar `v`: write the
            // field in place (copying the element only when it is shared).
            let write = match (ins, block.instrs.get(ii + 1), block.instrs.get(ii + 2)) {
                (
                    Instr::Take {
                        dst: t,
                        base,
                        proj: Proj::Index(i),
                    },
                    Some(Instr::Set {
                        base: b1,
                        proj: Proj::Field(k),
                        src: v,
                    }),
                    Some(Instr::Set {
                        base: b2,
                        proj: Proj::Index(i2),
                        src: t2,
                    }),
                ) if b1 == t
                    && t2 == t
                    && b2 == base
                    && i2 == i
                    && v != t
                    && v != base
                    && l.heap[t.0 as usize]
                    && !l.heap[v.0 as usize]
                    && dies[bi][ii].is_empty()
                    && dies[bi][ii + 1].is_empty()
                    && dies[bi][ii + 2].iter().all(|r| r == t) =>
                {
                    Some((*base, *i, *k as usize, *v))
                }
                _ => None,
            };
            if let Some((base, i, k, v)) = write {
                l.index_field_set(base, i, k, v)?;
                skip_more = 2;
                continue;
            }
            if let Some((base, i, d, k)) = fused {
                l.index_field(base, i, d, k as usize)?;
                skip_next = true;
                for r in std::mem::take(&mut l.dying) {
                    let old = l.get(r);
                    l.release(old);
                    l.clear(r);
                }
                continue;
            }
            l.instr(ins)?;
            // A heap register whose last use this was gives its reference back now.
            for r in std::mem::take(&mut l.dying) {
                if !l.moved.contains(&r) {
                    let old = l.get(r);
                    l.release(old);
                    l.clear(r);
                }
            }
        }
        match &block.term {
            Term::Jump(t) => {
                l.b.ins().jump(blocks[t.0 as usize], &[]);
            }
            Term::Branch { cond, then_, else_ } => {
                let c = l.get(*cond);
                l.b.ins().brif(
                    c,
                    blocks[then_.0 as usize],
                    &[],
                    blocks[else_.0 as usize],
                    &[],
                );
            }
            Term::Switch {
                value,
                cases,
                default,
            } => {
                let v = l.get(*value);
                let mut sw = cranelift_frontend::Switch::new();
                for (k, t) in cases {
                    sw.set_entry(*k as u128, blocks[t.0 as usize]);
                }
                sw.emit(l.b, v, blocks[default.0 as usize]);
            }
            Term::Return(r) => {
                let v = l.get(*r);
                if l.heap[r.0 as usize] {
                    // The reference moves to the caller.
                    l.clear(*r);
                }
                for i in 0..l.heap.len() {
                    if l.heap[i] {
                        let old = l.b.use_var(l.vars[i]);
                        l.release(old);
                    }
                }
                l.b.ins().return_(&[v]);
            }
            Term::Trap(msg) => {
                let id = l.msg(msg);
                let c = l.b.ins().iconst(types::I64, id);
                l.call("trap", &[c]);
                l.b.ins().trap(TrapCode::unwrap_user(1));
            }
        }
    }
    Ok(())
}
