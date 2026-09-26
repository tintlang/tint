use crate::ir::*;
use tint_ast::{Pattern, PatternField, Type};

pub struct IrVM {
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
}

impl IrVM {
    pub fn new(program: ProgramIR) -> Self {
        Self {
            program,
            locals: Default::default(),
            values: vec![],
            computed: Default::default(),
            current_instrs: Vec::new(),
            dst_index: Default::default(),
            native_call: None,
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

    fn alloc_value(&mut self, id: ValueId, v: Value) {
        if id as usize >= self.values.len() {
            self.values.resize((id + 1) as usize, Value::Unit);
        }
        self.values[id as usize] = v;
        self.computed.insert(id);
    }

    // Reads a value, computing it on demand if it hasn't run yet.
    //
    // Match-arm bodies get compiled into the flat instruction stream ahead of
    // the Match instruction that actually establishes their pattern bindings
    // (see `is_lazy`), so a plain forward pass can reach e.g. `LoadLocal("b")`
    // before "b" has been bound. Deferring those instructions and computing
    // them here, the first time they're actually needed, keeps them from
    // running before their bindings exist.
    fn get_value(&mut self, id: ValueId) -> Value {
        if !self.computed.contains(&id) {
            if let Some(&idx) = self.dst_index.get(&id) {
                let instr = self.current_instrs[idx].clone();
                self.exec_instr(&instr);
            }
        }
        self.values.get(id as usize).cloned().unwrap_or(Value::Unit)
    }

    // Instructions whose dst other instructions may reference as an operand.
    fn produced_dst(instr: &Instr) -> Option<ValueId> {
        match instr {
            Instr::Const { dst, .. }
            | Instr::LoadLocal { dst, .. }
            | Instr::Unary { dst, .. }
            | Instr::Binary { dst, .. }
            | Instr::Call { dst, .. }
            | Instr::FieldAccess { dst, .. }
            | Instr::NamespaceAccess { dst, .. }
            | Instr::Index { dst, .. }
            | Instr::Match { dst, .. }
            | Instr::StructInit { dst, .. }
            | Instr::StructUpdate { dst, .. }
            | Instr::VariantInit { dst, .. }
            | Instr::Array { dst, .. }
            | Instr::Tuple { dst, .. }
            | Instr::TupleExtract { dst, .. }
            | Instr::MapInit { dst, .. }
            | Instr::MapAccess { dst, .. } => Some(*dst),

            Instr::StoreLocal { .. }
            | Instr::FieldStore { .. }
            | Instr::IndexStore { .. }
            | Instr::Return(_) => None,
        }
    }

    // Pure, side-effect-free instructions are only computed the first time
    // something demands their value (via `get_value`), rather than
    // unconditionally in program order. Control/effectful instructions
    // (Match, Return, Call, StructUpdate, ...) still run eagerly in order.
    fn is_lazy(instr: &Instr) -> bool {
        matches!(
            instr,
            Instr::Const { .. }
                | Instr::LoadLocal { .. }
                | Instr::Unary { .. }
                | Instr::Binary { .. }
                | Instr::FieldAccess { .. }
                | Instr::NamespaceAccess { .. }
                | Instr::Index { .. }
                | Instr::StructInit { .. }
                | Instr::VariantInit { .. }
                | Instr::Array { .. }
                | Instr::Tuple { .. }
                | Instr::TupleExtract { .. }
                | Instr::MapInit { .. }
                | Instr::MapAccess { .. }
                // `Call` belongs here for the exact same reason `Binary`/etc.
                // do (see the module doc comment on match-arm bodies being
                // compiled ahead of the `Match` that establishes their
                // bindings): a recursive call sitting in an arm that ISN'T
                // taken must not run at all, let alone run unconditionally
                // on every level of the recursion. Before this, `Call` ran
                // eagerly in program order regardless of which arm `Match`
                // would go on to pick, so as soon as `Instr::Call` actually
                // did something (rather than the old no-op stub), a
                // recursive function's base case never had a chance to
                // stop it -- every level re-entered the recursive arm's
                // `Call` before `Match` ran at all, overflowing the stack
                // even for `factorial(0)`.
                | Instr::Call { .. }
        )
    }

    // Stringifies a value for "+" string concatenation (numbers/bools/etc.
    // get their plain text form, not their {:?} debug form).
    fn display_value(v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Unit => "()".to_string(),
            other => format!("{:?}", other),
        }
    }

    fn build_dst_index(instrs: &[Instr]) -> std::collections::HashMap<ValueId, usize> {
        let mut map = std::collections::HashMap::new();
        for (i, instr) in instrs.iter().enumerate() {
            if let Some(dst) = Self::produced_dst(instr) {
                map.insert(dst, i);
            }
        }
        map
    }

    // Recovers the function name a `Call`'s `func` operand refers to, for
    // the common `foo(a, b)` bare-identifier-callee shape. `func` is just
    // the ValueId of whatever instruction produced it; for a bare
    // identifier that instruction is a `LoadLocal { name, .. }` (see
    // `compiler.rs`'s `Expr::Call` lowering, which runs the callee
    // through the same generic `lower_expr` as any other value read).
    // Anything else (the ValueId came from a `FieldAccess`, a nested
    // `Call`, ...) isn't a plain named call and returns `None`.
    fn resolve_call_name(&self, func: ValueId) -> Option<String> {
        let idx = *self.dst_index.get(&func)?;
        match self.current_instrs.get(idx)? {
            Instr::LoadLocal { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    // Writes `new_value` back to whatever `target_id` actually reads from,
    // mirroring `TintVM::assign_to` in tint-runtime/src/vm.rs (which does
    // the exact same thing over the AST -- re-evaluate the target,
    // recurse to mutate it, write the mutated whole back to ITS own
    // target, bottoming out at a plain variable) but over this VM's flat
    // producer graph instead: a `ValueId` is just the output of whatever
    // instruction produced it, found the same way `resolve_call_name`
    // finds a `Call`'s callee.
    //
    // Used by `FieldStore`/`IndexStore` (`obj.field = x`, `arr[i] = x`),
    // which used to be `println!("WARNING: ... not implemented")` no-ops
    // -- the assignment parsed and ran, but silently changed nothing, so
    // `arr[1] = 10; arr[1]` still read the original value. Handles
    // arbitrary nesting (`a.b[0].c = x`) the same way the mirrored
    // AST-walk does: each level reads its own current value, mutates its
    // one field/index, and recurses to write the whole updated value back
    // to whatever produced ITS OWN `target_id`.
    fn write_back(&mut self, target_id: ValueId, new_value: Value) {
        // A NESTED target (`a.b[0].c = x`) needs to unwind first: mutate
        // the one field/index this level is responsible for, then recurse
        // to write the whole updated container back to whatever ITS OWN
        // `target_id` came from.
        if let Some(&idx) = self.dst_index.get(&target_id) {
            match self.current_instrs[idx].clone() {
                Instr::FieldAccess { base, field, .. } => {
                    let mut base_value = self.get_value(base);
                    if Self::set_field(&mut base_value, &field, new_value) {
                        self.write_back(base, base_value);
                    }
                    return;
                }

                Instr::Index { arr, index, .. } => {
                    let mut arr_value = self.get_value(arr);
                    let idx_value = self.get_value(index);
                    if Self::set_index(&mut arr_value, &idx_value, new_value) {
                        self.write_back(arr, arr_value);
                    }
                    return;
                }

                Instr::LoadLocal { name, .. } => {
                    self.locals.insert(name, new_value.clone());
                    // Fall through to also refresh this ValueId's own
                    // cached slot below (see why).
                }

                // Anything else -- a `StructInit`, an `Array`, the result
                // of a `Call`, ... -- falls through to the base case
                // below, which is what actually does the write.
                _ => {}
            }
        }

        // Base case. This is the ONLY place a write actually lands, and
        // it needs to handle two different shapes of "variable":
        //
        //   - A name bound via `Instr::StoreLocal`/read via a real
        //     `Instr::LoadLocal` (how a `Match` arm's pattern bindings
        //     work) -- the `LoadLocal` case above already updated
        //     `self.locals` by name for that.
        //   - A plain `let`/`let mut` binding, which -- see
        //     `compiler.rs`'s `Stmt::Let` lowering -- does NOT emit any
        //     `StoreLocal` at all: `bind_pattern`'s `Pattern::Ident` case
        //     (used for both `let x = ..` and `let mut x = ..` -- `mut`
        //     is not distinguished at compile time) just aliases the name
        //     directly to the ValueId that first computed its value, in
        //     the compile-time `locals: HashMap<String, ValueId>` cache
        //     `lower_expr`'s `Expr::Ident` consults. So `arr` in
        //     `let mut arr = [1,2,3]; arr[1] = 10;` never goes through
        //     `LoadLocal` at all -- every later use of `arr` resolves,
        //     at COMPILE time, straight to the `Array` instruction's own
        //     ValueId. There is no separate "slot" distinct from that
        //     ValueId to write into: the ValueId's cached value IS the
        //     variable's storage. So the general fix is to always refresh
        //     `target_id`'s own cached slot here, regardless of what
        //     instruction produced it -- `get_value` on that same ValueId
        //     later (via a fresh `LoadLocal`, or via the compile-time
        //     alias reusing it directly) then sees the update either way.
        self.alloc_value(target_id, new_value);
    }

    // In-place field update for `FieldStore`/`write_back`'s `StructInstance`
    // case. `false` means "not a struct, or no such field" -- the caller
    // decides what that means (currently: warn and leave the value alone,
    // matching how an out-of-bounds `IndexStore` is handled).
    fn set_field(target: &mut Value, field: &str, value: Value) -> bool {
        match target {
            Value::StructInstance { fields, .. } => {
                match fields.iter_mut().find(|(k, _)| k == field) {
                    Some(slot) => {
                        slot.1 = value;
                        true
                    }
                    None => false,
                }
            }
            // `m.field = value` on a map -- unlike a struct, a missing
            // key is inserted rather than rejected (mirrors the
            // tree-walking evaluator's `Value::set_field`, runtime/value/
            // methods.rs: `Value::Map(map) => { map.insert(..); }`, no
            // "field not found" case at all for maps).
            Value::Map(map) => {
                map.insert(field.to_string(), value);
                true
            }
            _ => false,
        }
    }

    // In-place index update for `IndexStore`/`write_back`'s `List`/`Tuple`
    // case. Mirrors `Instr::Index`'s read side (same two value kinds,
    // same numeric-index coercion).
    fn set_index(target: &mut Value, index: &Value, value: Value) -> bool {
        let idx = match index {
            Value::Number(n) => *n as usize,
            _ => return false,
        };

        match target {
            Value::List(items) | Value::Tuple(items) => {
                if idx < items.len() {
                    items[idx] = value;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // Calls another IR-compiled function by name with its own, fully
    // isolated call frame -- returns `None` if no function with that name
    // exists in this program (a native Rust function registered via
    // `TintVM::register_native`, or a builtin, isn't visible from here at
    // all: this VM only knows about IR-compiled `.tn` functions; the
    // caller decides what to do when this returns `None`).
    //
    // ValueIds are only unique WITHIN one function's flat instruction
    // list (see the module-level layout `locals`/`values`/`computed`/
    // `current_instrs`/`dst_index` are built around), so simply running
    // the callee against the CALLER's tables would silently clobber
    // whichever caller ValueId happens to collide with a callee one.
    // `std::mem::take` swaps in fresh, empty tables for the callee and
    // restores the caller's own afterward -- real Rust call-stack
    // recursion here gives each nesting level its own frame for free.
    fn call_named_function(&mut self, name: &str, args: &[Value]) -> Option<Value> {
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

    fn run_function(&mut self, func: &FunctionIR) -> Value {
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

    fn exec_instr(&mut self, instr: &Instr) -> Option<Value> {
        match instr {
            Instr::Const { dst, value } => {
                self.alloc_value(*dst, value.clone());
            }

            Instr::LoadLocal { dst, name } => {
                let v = self
                    .locals
                    .get(name)
                    .unwrap_or_else(|| panic!("Undefined variable {}", name))
                    .clone();

                self.alloc_value(*dst, v);
            }

            Instr::StoreLocal { name, src } => {
                let v = self.get_value(*src);
                self.locals.insert(name.clone(), v);
            }

            Instr::Unary { dst, op, src } => {
                let v = self.get_value(*src).unwrap_number();
                let out = match op.as_str() {
                    "-" => -v,
                    "+" => v,
                    _ => panic!("Unknown unary op {}", op),
                };
                self.alloc_value(*dst, Value::Number(out));
            }

            Instr::Binary { dst, op, lhs, rhs } => {
                let l = self.get_value(*lhs);
                let r = self.get_value(*rhs);

                // "+" also means string concatenation once either side
                // is a String (e.g. `"" + id + ":" + name`).
                let out = if op == "+"
                    && (matches!(l, Value::String(_)) || matches!(r, Value::String(_)))
                {
                    Value::String(format!(
                        "{}{}",
                        Self::display_value(&l),
                        Self::display_value(&r)
                    ))
                } else {
                    // Comparisons and boolean ops -- previously entirely
                    // unhandled here (only "+"/"-"/"*"/"/" existed, and
                    // both operands got force-unwrapped as numbers before
                    // the op was even looked at, so e.g. a guard's `x > 10`
                    // would have panicked in `unwrap_number` before ever
                    // reaching the `_ => panic!("Unknown binary op")`
                    // below). Mirrors the already-correct handling in the
                    // tree-walking evaluator (`tint-evaluator`'s
                    // `evaluator/expr.rs`, `Expr::Binary`) op-for-op, so
                    // the two execution paths agree on what each operator
                    // does: `<`/`>`/`<=`/`>=` are numeric-only; `==`/`!=`
                    // also cover Bool and String; falling through to
                    // `Value::Unit` (not a panic) for any other
                    // operator/operand-type combination matches that same
                    // reference behavior.
                    match (&l, op.as_str(), &r) {
                        (Value::Number(a), "+", Value::Number(b)) => Value::Number(a + b),
                        (Value::Number(a), "-", Value::Number(b)) => Value::Number(a - b),
                        (Value::Number(a), "*", Value::Number(b)) => Value::Number(a * b),
                        (Value::Number(a), "/", Value::Number(b)) => Value::Number(a / b),

                        (Value::Bool(a), "&&", Value::Bool(b)) => Value::Bool(*a && *b),
                        (Value::Bool(a), "||", Value::Bool(b)) => Value::Bool(*a || *b),

                        (Value::Number(a), "<", Value::Number(b)) => Value::Bool(a < b),
                        (Value::Number(a), ">", Value::Number(b)) => Value::Bool(a > b),
                        (Value::Number(a), "<=", Value::Number(b)) => Value::Bool(a <= b),
                        (Value::Number(a), ">=", Value::Number(b)) => Value::Bool(a >= b),

                        (Value::Number(a), "==", Value::Number(b)) => Value::Bool(a == b),
                        (Value::Number(a), "!=", Value::Number(b)) => Value::Bool(a != b),
                        (Value::Bool(a), "==", Value::Bool(b)) => Value::Bool(a == b),
                        (Value::Bool(a), "!=", Value::Bool(b)) => Value::Bool(a != b),
                        (Value::String(a), "==", Value::String(b)) => Value::Bool(a == b),
                        (Value::String(a), "!=", Value::String(b)) => Value::Bool(a != b),

                        _ => Value::Unit,
                    }
                };

                self.alloc_value(*dst, out);
            }

            Instr::Call { dst, func, args } => {
                // A bare-identifier callee (`foo(a, b)`) compiles `foo`
                // through the same generic expression-lowering path as
                // any other value read, so `func` here is the ValueId of
                // a `LoadLocal { name: "foo", .. }` -- see
                // `resolve_call_name`. Anything else (calling a value
                // read from a variable, e.g. a lambda stored in a local)
                // isn't resolvable to a function name this way and stays
                // unsupported, same as before this fix.
                let callee_name = self.resolve_call_name(*func);

                // Arguments must be read BEFORE the call-frame swap below
                // -- they live in the CALLER's value table.
                let arg_values: Vec<Value> = args.iter().map(|a| self.get_value(*a)).collect();

                let result = match callee_name {
                    Some(name) => match self.call_named_function(&name, &arg_values) {
                        Some(v) => v,
                        None => {
                            println!(
                                "WARNING: Call to unknown function `{}`, returning unit",
                                name
                            );
                            Value::Unit
                        }
                    },
                    None => {
                        println!(
                            "WARNING: Call to a non-function-name callee is not supported yet, returning unit"
                        );
                        Value::Unit
                    }
                };

                self.alloc_value(*dst, result);
            }

            Instr::FieldAccess { dst, base, field } => {
                let obj = self.get_value(*base);
                match obj {
                    Value::StructInstance { ref fields, .. } => {
                        if let Some((_, v)) = fields.iter().find(|(k, _)| k == field) {
                            self.alloc_value(*dst, v.clone());
                        } else {
                            panic!("Field {} not found in struct", field);
                        }
                    }
                    Value::Map(ref m) => {
                        if let Some(v) = m.get(field) {
                            self.alloc_value(*dst, v.clone());
                        } else {
                            panic!("Key {} not found in map", field);
                        }
                    }
                    _ => panic!("Field access on non-struct/map: {:?}", obj),
                }
            }

            Instr::NamespaceAccess { dst, .. } => {
                println!("WARNING: namespace access not implemented");
                self.alloc_value(*dst, Value::Unit);
            }

            Instr::Index { dst, arr, index } => {
                let array = self.get_value(*arr);
                let idx = self.get_value(*index).unwrap_number() as usize;
                match array {
                    Value::List(ref items) | Value::Tuple(ref items) => {
                        if idx < items.len() {
                            self.alloc_value(*dst, items[idx].clone());
                        } else {
                            panic!("Index {} out of bounds", idx);
                        }
                    }
                    _ => panic!("Index access on non-list/tuple: {:?}", array),
                }
            }

            Instr::FieldStore { base, field, src } => {
                // `obj.field = value` -- used to be a `println!` no-op:
                // the assignment ran but changed nothing at all, so
                // `u.age = 20; u.age` still read the original value.
                // Fixed via `write_back` (see its doc comment) -- reads
                // `base`'s current struct value, updates the one field,
                // and writes the whole updated struct back to whatever
                // `base` actually came from.
                let value = self.get_value(*src);
                let mut base_value = self.get_value(*base);

                if Self::set_field(&mut base_value, field, value) {
                    self.write_back(*base, base_value);
                } else {
                    println!(
                        "WARNING: field store target has no field `{}`, or isn't a struct",
                        field
                    );
                }
            }

            Instr::IndexStore { arr, index, src } => {
                // `arr[i] = value` -- same fix as `FieldStore` above, for
                // list/tuple indexing instead of a struct field.
                let value = self.get_value(*src);
                let mut arr_value = self.get_value(*arr);
                let idx_value = self.get_value(*index);

                if Self::set_index(&mut arr_value, &idx_value, value) {
                    self.write_back(*arr, arr_value);
                } else {
                    println!(
                        "WARNING: index store target isn't a list/tuple, or index is out of bounds/not a number"
                    );
                }
            }

            Instr::Match {
                dst,
                scrutinee,
                arms,
            } => {
                let scrutinee_value = self.get_value(*scrutinee);

                // Try to match against each arm
                let mut matched = false;
                for (pattern, guard, result_id) in arms {
                    // Try to match the pattern
                    if let Some(bindings) = self.match_pattern(pattern, &scrutinee_value) {
                        // Bind all variables from the pattern match BEFORE
                        // evaluating the guard -- a guard almost always
                        // references the pattern's own bound names (`x if
                        // x > 10`), and this is the only place they exist
                        // yet. This used to be a hardcoded `true` with a
                        // `// TODO: evaluate guard expression`, so a guard
                        // had literally no effect on which arm ran; now it
                        // actually demands the guard's value.
                        for (var_name, var_value) in bindings {
                            self.locals.insert(var_name, var_value);
                        }

                        let guard_passes = match guard {
                            Some(guard_id) => match self.get_value(*guard_id) {
                                Value::Bool(b) => b,
                                other => panic!(
                                    "match guard must evaluate to a bool, got {:?}",
                                    other
                                ),
                            },
                            None => true,
                        };

                        if guard_passes {
                            // Execute the result value
                            let result = self.get_value(*result_id);
                            self.alloc_value(*dst, result);
                            matched = true;
                            break;
                        }
                        // Guard failed: fall through and try the next arm.
                        // The bindings just installed are simply
                        // overwritten (or left, harmlessly) by whatever
                        // arm matches next -- the same flat, single
                        // namespace every other binding in this function
                        // already lives with (see `compiler.rs`'s
                        // `Expr::Match` lowering comment).
                    }
                }

                if !matched {
                    // No arm matched - return unit or could panic
                    self.alloc_value(*dst, Value::Unit);
                }
            }

            Instr::StructInit {
                dst, name, fields, ..
            } => {
                let mut field_values = Vec::new();
                for (field_name, field_id) in fields {
                    field_values.push((field_name.clone(), self.get_value(*field_id)));
                }
                self.alloc_value(
                    *dst,
                    Value::StructInstance {
                        name: name.clone(),
                        fields: field_values,
                    },
                );
            }

            Instr::VariantInit {
                dst,
                enum_name,
                variant,
                fields,
            } => {
                // Keep the field NAME alongside each value now (see
                // `Value::EnumInstance`'s doc comment in ir.rs) -- this
                // used to discard `name` and keep only a positional
                // `Vec<Value>`, which made a struct-style variant
                // pattern (`A { x } => ..`) impossible to match by field
                // name later.
                let mut field_values = Vec::new();

                for (name, src) in fields {
                    let v = self.get_value(*src);
                    field_values.push((name.clone(), v));
                }

                let v = Value::EnumInstance {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields: field_values,
                };

                self.alloc_value(*dst, v);
            }

            Instr::StructUpdate { dst, base, updates } => {
                // `Base { field: expr, ..base }` -- a spread-update EXPRESSION
                // that builds a NEW struct value (unlike `FieldStore`
                // above, which mutates an existing variable in place).
                // Mirrors the already-correct tree-walking evaluator
                // (`tint-evaluator`'s `evaluator/expr.rs`,
                // `Expr::StructUpdate`): panics on a non-struct base (same
                // as it does), overrides an existing field or appends a
                // new one. Used to be a `println!` stub that always
                // returned `Unit`, so e.g. `User { age{u.age+1}, ..u }`
                // silently produced `Unit` instead of the updated `User`.
                let base_value = self.get_value(*base);
                let (name, mut fields) = match base_value {
                    Value::StructInstance { name, fields } => (name, fields),
                    other => panic!("struct update base is not a struct instance: {:?}", other),
                };

                for (field_name, src) in updates {
                    let v = self.get_value(*src);
                    match fields.iter_mut().find(|(k, _)| k == field_name) {
                        Some(slot) => slot.1 = v,
                        None => fields.push((field_name.clone(), v)),
                    }
                }

                self.alloc_value(*dst, Value::StructInstance { name, fields });
            }

            Instr::Tuple { dst, items } => {
                let mut out = Vec::new();
                for id in items {
                    out.push(self.get_value(*id));
                }
                self.alloc_value(*dst, Value::Tuple(out));
            }

            Instr::TupleExtract { dst, tuple, index } => {
                let v = self.get_value(*tuple);
                match v {
                    Value::Tuple(items) => {
                        let elem = items
                            .get(*index)
                            .unwrap_or_else(|| panic!("Tuple index {} out of bounds", index))
                            .clone();

                        self.alloc_value(*dst, elem);
                    }
                    other => panic!("TupleExtract on non-tuple value: {:?}", other),
                }
            }

            Instr::Array { dst, items } => {
                let mut out = Vec::new();
                for id in items {
                    out.push(self.get_value(*id));
                }
                self.alloc_value(*dst, Value::List(out));
            }

            Instr::MapInit { dst, entries } => {
                let mut m = std::collections::HashMap::new();
                for (k, id) in entries {
                    m.insert(k.clone(), self.get_value(*id));
                }
                self.alloc_value(*dst, Value::Map(m));
            }

            Instr::MapAccess { dst, map, key } => {
                let m = self.get_value(*map);

                match m {
                    Value::Map(ref hm) => {
                        // HashMap<String, Value>, key: String -> hm.get(&key)
                        if let Some(v) = hm.get(key) {
                            self.alloc_value(*dst, v.clone());
                        } else {
                            panic!("Map missing key `{}`", key);
                        }
                    }
                    other => panic!("MapAccess on non-map: {:?}", other),
                }
            }

            Instr::Return(id) => {
                return Some(self.get_value(*id));
            }
        }

        None
    }

    // Shared by `Pattern::Struct` against either a `StructInstance`'s or an
    // `EnumInstance`'s named fields (see the doc comment where this is
    // called): walks the pattern's field list against a `(name, value)`
    // slice, binding/recursing for `Shorthand`/`Assign` and skipping `Rest`.
    fn match_named_fields(
        &self,
        pattern_fields: &[PatternField],
        value_fields: &[(String, Value)],
        bindings: &mut std::collections::HashMap<String, Value>,
    ) -> bool {
        for field_pattern in pattern_fields {
            match field_pattern {
                PatternField::Shorthand { field, .. } => {
                    if let Some((_, v)) = value_fields.iter().find(|(k, _)| k == field) {
                        bindings.insert(field.clone(), v.clone());
                    } else {
                        return false;
                    }
                }
                PatternField::Assign { field, pat, .. } => {
                    if let Some((_, v)) = value_fields.iter().find(|(k, _)| k == field) {
                        if !self.pattern_matches(pat, v, bindings) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                PatternField::Rest(_) => {
                    // Rest pattern matches remaining fields
                }
            }
        }
        true
    }

    // Pattern matching helper functions
    fn match_pattern(
        &self,
        pattern: &Pattern,
        value: &Value,
    ) -> Option<std::collections::HashMap<String, Value>> {
        let mut bindings = std::collections::HashMap::new();

        if self.pattern_matches(pattern, value, &mut bindings) {
            Some(bindings)
        } else {
            None
        }
    }

    fn pattern_matches(
        &self,
        pattern: &Pattern,
        value: &Value,
        bindings: &mut std::collections::HashMap<String, Value>,
    ) -> bool {
        match pattern {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything without binding
                true
            }

            Pattern::Ident(name, _) => {
                // Ident matches anything and binds it
                bindings.insert(name.clone(), value.clone());
                true
            }

            Pattern::Number(num_str, _) => {
                // Match a specific number
                if let Ok(n) = num_str.parse::<f64>() {
                    if let Value::Number(v) = value {
                        *v == n
                    } else {
                        false
                    }
                } else {
                    false
                }
            }

            Pattern::String(s, _) => {
                // Match a specific string
                if let Value::String(v) = value {
                    v == s
                } else {
                    false
                }
            }

            Pattern::Tuple(patterns, _) => {
                // Match a tuple - recursively match each element
                if let Value::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return false;
                    }

                    for (pat, val) in patterns.iter().zip(values.iter()) {
                        if !self.pattern_matches(pat, val, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }

            Pattern::Struct { name, fields, .. } => {
                // `Name { field, .. }` -- matches a real struct BY NAME,
                // or (see the doc comment on `Value::EnumInstance`) a
                // struct-STYLE enum variant matched BY VARIANT name, since
                // the parser produces this same `Pattern::Struct` for both
                // shapes (`tint-parser`'s `parse_ident_based_pattern` can't
                // tell a struct name from a variant name apart at a bare
                // `Name {` pattern site) and both value kinds now carry
                // named fields the same way. `A { x } => ..` against
                // `E2::A { x { 50 } }` used to fall through to `_` every
                // time, matching neither branch below.
                match value {
                    Value::StructInstance {
                        name: struct_name,
                        fields: struct_fields,
                    } => {
                        if struct_name != name {
                            return false;
                        }
                        self.match_named_fields(fields, struct_fields, bindings)
                    }

                    Value::EnumInstance {
                        variant,
                        fields: enum_fields,
                        ..
                    } => {
                        if variant != name {
                            return false;
                        }
                        self.match_named_fields(fields, enum_fields, bindings)
                    }

                    _ => false,
                }
            }

            Pattern::Variant { name, args, .. } => {
                // `Name(a, b)` -- positional. `EnumInstance::fields` is
                // still stored in declaration order, so zipping against it
                // positionally (ignoring the names) is exactly the same
                // match this always did, just read off the renamed field.
                if let Value::EnumInstance {
                    variant,
                    fields: variant_fields,
                    ..
                } = value
                {
                    if variant != name {
                        return false;
                    }

                    if args.len() != variant_fields.len() {
                        return false;
                    }

                    for (pat, (_, val)) in args.iter().zip(variant_fields.iter()) {
                        if !self.pattern_matches(pat, val, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }

            // `x: i32 => ...` -- was previously caught by the `_ => true`
            // fallback below, meaning it matched UNCONDITIONALLY (no type
            // check at all) and bound NOTHING, so the arm body silently
            // read whatever the name already held from outer scope instead
            // of the actual scrutinee. Mirrors the already-correct
            // handling in tint-evaluator (runtime/host_vm/patterns.rs /
            // value/methods.rs's `matches_type`): check the runtime type,
            // then delegate to the inner pattern so the name is actually
            // bound to the scrutinee.
            Pattern::Typed { pat, ty, .. } => {
                Self::value_matches_type(value, ty) && self.pattern_matches(pat, value, bindings)
            }

            // Struct-like enum variants (`enum E { A { x } }`, matched as
            // `A { x } => ...`) used to be a known gap here -- `Name {`
            // always parses to `Pattern::Struct` (the parser can't tell a
            // struct name from a variant name apart at that bare syntax;
            // see tint-parser's `parse_ident_based_pattern`), and that only
            // ever matched `Value::StructInstance`, never
            // `Value::EnumInstance`. Fixed above: `Pattern::Struct` now
            // accepts either value kind (matching a `StructInstance` by
            // struct name or an `EnumInstance` by variant name), which
            // needed `Value::EnumInstance` to retain field NAMES instead of
            // a positional `Vec<Value>` -- see its doc comment in ir.rs.
            // This IR VM is the only place that needed fixing: this crate's
            // `Value` is private to it, so the tree-walking evaluator
            // (`tint-evaluator`, its own separate `Value`/pattern-matching
            // code) is untouched by this change and still has the
            // equivalent restriction on its own execution path.
            _ => true,
        }
    }

    // Runtime type check for `Pattern::Typed` (`x: i32`). Deliberately
    // mirrors tint-evaluator's `Value::matches_type` so the two execution
    // paths agree on what a type name means at runtime -- see that
    // function (runtime/value/methods.rs) for the reference behavior this
    // is kept in sync with.
    fn value_matches_type(value: &Value, ty: &Type) -> bool {
        match ty {
            Type::Simple(t) => match value {
                Value::Number(_) => t == "i32" || t == "f32" || t == "f64" || t == "number",
                Value::Bool(_) => t == "bool",
                Value::String(_) => t == "string",
                Value::Unit => false,
                Value::StructInstance { name, .. } => name == t,
                Value::EnumInstance { enum_name, .. } => enum_name == t,
                _ => false,
            },
            Type::Unit => matches!(value, Value::Unit),
            // Generic type arguments aren't enforced by the runtime yet,
            // same as the evaluator.
            Type::Generic(_, _) => true,
            Type::Union(_) => true,
        }
    }
}
