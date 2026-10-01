impl Jit {
    /// Runs `main` and returns its result rendered like the interpreter would.
    pub fn run_main(&self) -> Result<String, String> {
        self.run_fn("main")
    }

    /// Runs the function `name` (no parameters) and returns its result
    /// rendered like the interpreter would. The function must have been
    /// compiled (named in `compile_for`, or reachable from it).
    pub fn run_fn(&self, name: &str) -> Result<String, String> {
        let fid = *self
            .module
            .functions
            .get(name)
            .ok_or_else(|| format!("no function `{name}`"))?;
        let id = self.ids[fid.0 as usize].ok_or_else(|| format!("`{name}` was not compiled"))?;
        rt::enter(&self.module, &self.msgs);
        if let (Some(init), false) = (self.module.init, self.init_done.get()) {
            // Globals the program never touches leave `init` uncompiled.
            if let Some(iid) = self.ids[init.0 as usize] {
                let p = self.jit.get_finalized_function(iid);
                let f: extern "C" fn() -> u8 = unsafe { std::mem::transmute(p) };
                f();
            }
            self.init_done.set(true);
        }
        let ty = self.module.func(fid).ret;
        let p = self.jit.get_finalized_function(id);
        // Floats come back in a different register than integers.
        let bits = if clif_ty(&self.module, ty) == Ok(types::F64) {
            let f: extern "C" fn() -> f64 = unsafe { std::mem::transmute(p) };
            f().to_bits()
        } else {
            let f: extern "C" fn() -> u64 = unsafe { std::mem::transmute(p) };
            f()
        };
        Ok(rt::render_bits(&self.module, ty, bits))
    }
}
