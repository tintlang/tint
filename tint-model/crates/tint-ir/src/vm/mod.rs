use crate::ir::*;
use tint_ast::Pattern;

mod calls;
mod execution;
mod helpers;
mod patterns;

pub struct IrVM<'a> {
    pub program: ProgramIR,
    locals: std::collections::HashMap<String, Value>,
    values: Vec<Value>,                                   // SSA value table
    computed: std::collections::HashSet<ValueId>,         // which dsts have actually been computed
    current_instrs: Vec<Instr>, // instrs of the function currently executing
    dst_index: std::collections::HashMap<ValueId, usize>, // dst -> index in current_instrs

    // Optional escape hatch to whatever native-Rust-function table the
    // embedder keeps (`TintVM::register_native`/`native_fns` in
    // tint-runtime) -- this crate has no dependency on tint-runtime, so it
    // can't reach that table directly; the embedder installs a callback
    // here instead (see `set_native_call`). `None` (the default, e.g. in
    // every test that builds an `IrVM` directly) just means "no native
    // fallback available", not an error -- `call_named_function` treats
    // it exactly like "no such function".
    native_call: Option<Box<dyn Fn(&str, &[Value]) -> Option<Value>>>,
    method_call: Option<&'a mut dyn FnMut(Value, &str, &[Value]) -> Option<(Value, Value)>>,
}

impl<'a> IrVM<'a> {
    pub fn new(program: ProgramIR) -> Self {
        Self {
            program,
            locals: Default::default(),
            values: vec![],
            computed: Default::default(),
            current_instrs: Vec::new(),
            dst_index: Default::default(),
            native_call: None,
            method_call: None,
        }
    }

    // Installs the native-fn fallback `call_named_function` tries once a
    // `Call`'s callee name isn't any IR-compiled `.tn` function -- this is
    // what closes the gap described in `tint-runtime/tests/native_fn.rs`'s
    // module doc comment: a native fn is now reachable from a plain `fn`
    // invoked the normal top-level way (`TintVM::call_fn`'s IR branch),
    // not just from click/hover handlers and other tree-walked call
    // sites. The callback's own `Option<Value>` return means "no native
    // registered under this name" (as opposed to this method's `None`,
    // which additionally covers "no callback installed at all").
    pub fn set_native_call(&mut self, cb: Box<dyn Fn(&str, &[Value]) -> Option<Value>>) {
        self.native_call = Some(cb);
    }

    pub fn set_method_call(
        &mut self,
        cb: &'a mut dyn FnMut(Value, &str, &[Value]) -> Option<(Value, Value)>,
    ) {
        self.method_call = Some(cb);
    }

    pub fn run(&mut self, entry: &str) -> Value {
        self.run_with_args(entry, &[], &[])
    }

    // Same as `run`, but binds each `params[i]` pattern to `args[i]` (as
    // locals) before executing — this is how function arguments reach the
    // IR VM, which otherwise has no notion of a call's arguments.
    pub fn run_with_args(&mut self, entry: &str, params: &[Pattern], args: &[Value]) -> Value {
        let index = self
            .program
            .functions
            .iter()
            .position(|f| f.name == entry)
            .expect("function not found");

        // no active borrow now (we cloned func above)
        let func = self.program.functions[index].clone();

        for (param, arg) in params.iter().zip(args.iter()) {
            if let Some(bindings) = self.match_pattern(param, arg) {
                for (name, v) in bindings {
                    self.locals.insert(name, v);
                }
            }
        }

        self.run_function(&func)
    }
}
