use super::*;

impl IrVM {
    pub(super) fn alloc_value(&mut self, id: ValueId, v: Value) {
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
    pub(super) fn get_value(&mut self, id: ValueId) -> Value {
        if !self.computed.contains(&id) {
            if let Some(&idx) = self.dst_index.get(&id) {
                let instr = self.current_instrs[idx].clone();
                self.exec_instr(&instr);
            }
        }
        self.values.get(id as usize).cloned().unwrap_or(Value::Unit)
    }

    // Instructions whose dst other instructions may reference as an operand.
    pub(super) fn produced_dst(instr: &Instr) -> Option<ValueId> {
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
    pub(super) fn is_lazy(instr: &Instr) -> bool {
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
    pub(super) fn display_value(v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Unit => "()".to_string(),
            other => format!("{:?}", other),
        }
    }

    pub(super) fn build_dst_index(instrs: &[Instr]) -> std::collections::HashMap<ValueId, usize> {
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
    pub(super) fn resolve_call_name(&self, func: ValueId) -> Option<String> {
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
    pub(super) fn write_back(&mut self, target_id: ValueId, new_value: Value) {
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
    pub(super) fn set_field(target: &mut Value, field: &str, value: Value) -> bool {
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
    pub(super) fn set_index(target: &mut Value, index: &Value, value: Value) -> bool {
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
}
