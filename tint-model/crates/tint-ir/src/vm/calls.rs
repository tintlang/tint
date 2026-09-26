use super::*;

impl IrVM {
    pub(super) fn call_named_function(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        let idx = match self.program.functions.iter().position(|f| f.name == name) {
            Some(idx) => idx,
            // Not an IR-compiled `.tn` function -- try the native-fn
            // fallback (see `set_native_call`) before giving up. No
            // call-frame isolation needed here: a native fn is a plain
            // Rust closure operating only on `args`/its return value, not
            // on any of this VM's own instruction/local-variable state.
            None => return self.native_call.as_ref().and_then(|cb| cb(name, args)),
        };
        let func_ir = self.program.functions[idx].clone();

        let saved_locals = std::mem::take(&mut self.locals);
        let saved_values = std::mem::take(&mut self.values);
        let saved_computed = std::mem::take(&mut self.computed);
        let saved_instrs = std::mem::take(&mut self.current_instrs);
        let saved_dst_index = std::mem::take(&mut self.dst_index);

        for (param, arg) in func_ir.params.iter().zip(args.iter()) {
            if let Some(bindings) = self.match_pattern(param, arg) {
                for (n, v) in bindings {
                    self.locals.insert(n, v);
                }
            }
        }

        let result = self.run_function(&func_ir);

        self.locals = saved_locals;
        self.values = saved_values;
        self.computed = saved_computed;
        self.current_instrs = saved_instrs;
        self.dst_index = saved_dst_index;

        Some(result)
    }

    pub(super) fn run_function(&mut self, func: &FunctionIR) -> Value {
        // Run only block 0 for now
        let block = &func.blocks[0];

        self.current_instrs = block.instrs.clone();
        self.dst_index = Self::build_dst_index(&self.current_instrs);

        let instrs = self.current_instrs.clone();
        for instr in instrs.iter() {
            if let Some(dst) = Self::produced_dst(instr) {
                if self.computed.contains(&dst) {
                    // Already computed on demand while evaluating a later
                    // (in program order) instruction that depended on it.
                    continue;
                }
                if Self::is_lazy(instr) {
                    // Deferred until something actually needs its value.
                    continue;
                }
            }

            if let Some(ret) = self.exec_instr(instr) {
                return ret;
            }
        }

        // No explicit `Return` ran — fall back to the value of the block's
        // last instruction (e.g. a trailing `match` used as the function's
        // result), like an implicit return of the last expression.
        if let Some(last) = instrs.last() {
            if let Some(dst) = Self::produced_dst(last) {
                return self.get_value(dst);
            }
        }

        Value::Unit
    }
}
