use super::*;

// ---- thunks and start ------------------------------------------------------------------------------------

/// The entry point closures are called through: `(closure, declared params...)`.
/// It unpacks the captured values from the closure object and calls the real
/// function, which takes them as its first parameters.
pub fn thunk(cx: &mut Cx<'_>, f: &Func, real: u32) -> Function {
    let m = cx.m;
    let nparams = 1 + (f.params.len() - f.ncaptures as usize) as u32;
    let mut code: Vec<I<'static>> = Vec::new();
    for i in 0..f.ncaptures as usize {
        let ty = f.reg_ty(f.params[i]);
        code.push(I::LocalGet(0));
        match val_type(m, ty) {
            ValType::F64 => code.push(I::F64Load(ma(slot_off(1 + i), 3))),
            ValType::I64 => code.push(I::I64Load(ma(slot_off(1 + i), 3))),
            _ => code.push(I::I32Load(ma(slot_off(1 + i), 2))),
        }
        if is_heap(m, ty) {
            // The callee owns its parameters; the closure keeps its own reference.
            let t = nparams; // scratch local
            code.push(I::LocalSet(t));
            code.push(I::LocalGet(t));
            code.push(I::If(BlockType::Empty));
            code.push(I::LocalGet(t));
            code.push(I::LocalGet(t));
            code.push(I::I32Load(ma(OFF_RC, 2)));
            code.push(I::I32Const(1));
            code.push(I::I32Add);
            code.push(I::I32Store(ma(OFF_RC, 2)));
            code.push(I::End);
            code.push(I::LocalGet(t));
        }
    }
    for i in 1..nparams {
        code.push(I::LocalGet(i));
    }
    code.push(I::Call(real));
    code.push(I::End);
    let mut func = Function::new([(1, I32)]);
    for i in &code {
        func.instruction(i);
    }
    func
}

/// `tint:run_callback(closure, argument)`: how the host answers a callback it
/// kept (a settled Promise). The argument is owned by the callee.
pub fn run_callback(cx: &mut Cx<'_>) -> Function {
    let ty = cx.func_type(vec![I32, I32], vec![I32]);
    let mut f = Function::new([]);
    for i in [
        I::LocalGet(0),
        I::LocalGet(1),
        I::LocalGet(0),
        I::I64Load(ma(slot_off(0), 3)),
        I::I32WrapI64,
        I::CallIndirect {
            type_index: ty,
            table_index: 0,
        },
        I::Drop,
        I::End,
    ] {
        f.instruction(&i);
    }
    f
}

/// The module's start function: copies the static data into the runtime's
/// memory, hands the descriptor to the runtime, creates the literal and
/// variant objects and runs the global initializers.
pub fn start(
    cx: &mut Cx<'_>,
    desc_len: u32,
    data_len: u32,
    lits: &HashMap<usize, (u32, u32)>,
    init: Option<u32>,
) -> Function {
    let mut f = Function::new([(1, I32)]);
    let base = 0u32;
    let nmod = cx.m.globals.len() as u32;
    let mut ins = |i: I<'_>| {
        f.instruction(&i);
    };
    // base = rt_alloc(data_len)
    ins(I::I32Const(data_len as i32));
    ins(I::Call(cx.imp(Imp::RtAlloc)));
    ins(I::LocalSet(base));
    ins(I::LocalGet(base));
    ins(I::I32Const(0));
    ins(I::I32Const(data_len as i32));
    ins(I::MemoryInit {
        mem: 0,
        data_index: 0,
    });
    ins(I::DataDrop(0));
    ins(I::LocalGet(base));
    ins(I::I32Const(desc_len as i32));
    ins(I::Call(cx.imp(Imp::RtInit)));
    let extras: Vec<ExtraKey> = cx.extras().to_vec();
    for (i, e) in extras.iter().enumerate() {
        match e {
            ExtraKey::Lit(_) => {
                let (off, len) = lits[&i];
                ins(I::LocalGet(base));
                ins(I::I32Const((desc_len + off) as i32));
                ins(I::I32Add);
                ins(I::I32Const(len as i32));
                ins(I::Call(cx.imp(Imp::StrLit)));
            }
            ExtraKey::Variant(n, tag) => {
                ins(I::I32Const(*n as i32));
                ins(I::I32Const(*tag as i32));
                ins(I::Call(cx.imp(Imp::ImmortalVariant)));
            }
        }
        ins(I::GlobalSet(nmod + i as u32));
    }
    if let Some(init) = init {
        ins(I::Call(init));
        ins(I::Drop);
    }
    ins(I::End);
    f
}
