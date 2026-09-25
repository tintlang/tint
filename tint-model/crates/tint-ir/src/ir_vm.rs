use crate::ir::*;
use tint_ast::{Pattern, PatternField};

pub struct IrVM {
    pub program: ProgramIR,
    locals: std::collections::HashMap<String, Value>,
    values: Vec<Value>, // SSA value table
    computed: std::collections::HashSet<ValueId>, // which dsts have actually been computed
    current_instrs: Vec<Instr>, // instrs of the function currently executing
    dst_index: std::collections::HashMap<ValueId, usize>, // dst -> index in current_instrs
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
        }
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

        // теперь нет активного заимствования
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
                    let v = self.locals.get(name)
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
                        "+" =>  v,
                        _ => panic!("Unknown unary op {}", op),
                    };
                    self.alloc_value(*dst, Value::Number(out));
                }

                Instr::Binary { dst, op, lhs, rhs } => {
                    let l = self.get_value(*lhs);
                    let r = self.get_value(*rhs);

                    // "+" also means string concatenation once either side
                    // is a String (e.g. `"" + id + ":" + name`).
                    let out = if op == "+" && (matches!(l, Value::String(_)) || matches!(r, Value::String(_))) {
                        Value::String(format!("{}{}", Self::display_value(&l), Self::display_value(&r)))
                    } else {
                        let l = l.unwrap_number();
                        let r = r.unwrap_number();

                        let out = match op.as_str() {
                            "+" => l + r,
                            "-" => l - r,
                            "*" => l * r,
                            "/" => l / r,
                            _ => panic!("Unknown binary op {}", op),
                        };

                        Value::Number(out)
                    };

                    self.alloc_value(*dst, out);
                }

                Instr::Call { dst, func, args } => {
                    println!("WARNING: Call not implemented, returning unit");
                    self.alloc_value(*dst, Value::Unit);
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

                Instr::FieldStore { .. } => {
                    println!("WARNING: field store not implemented");
                }

                Instr::IndexStore { .. } => {
                    println!("WARNING: index store not implemented");
                }

                Instr::Match { dst, scrutinee, arms } => {
                    let scrutinee_value = self.get_value(*scrutinee);
                    
                    // Try to match against each arm
                    let mut matched = false;
                    for (pattern, guard, result_id) in arms {
                        // Try to match the pattern
                        if let Some(bindings) = self.match_pattern(pattern, &scrutinee_value) {
                            // Pattern matched, check guard if present
                            let guard_passes = if let Some(_guard_id) = guard {
                                // TODO: evaluate guard expression
                                true
                            } else {
                                true
                            };
                            
                            if guard_passes {
                                // Bind all variables from pattern match
                                for (var_name, var_value) in bindings {
                                    self.locals.insert(var_name, var_value);
                                }
                                
                                // Execute the result value
                                let result = self.get_value(*result_id);
                                self.alloc_value(*dst, result);
                                matched = true;
                                break;
                            }
                        }
                    }
                    
                    if !matched {
                        // No arm matched - return unit or could panic
                        self.alloc_value(*dst, Value::Unit);
                    }
                }

                Instr::StructInit { dst, name, fields, .. } => {
                    let mut field_values = Vec::new();
                    for (field_name, field_id) in fields {
                        field_values.push((field_name.clone(), self.get_value(*field_id)));
                    }
                    self.alloc_value(*dst, Value::StructInstance { name: name.clone(), fields: field_values });
                }

                Instr::VariantInit { dst, enum_name, variant, fields } => {
                    let mut args = Vec::new();

                    for (_, src) in fields {
                        let v = self.get_value(*src);
                        args.push(v);
                    }

                    let v = Value::EnumInstance {
                        enum_name: enum_name.clone(),
                        variant: variant.clone(),
                        args,
                    };

                    self.alloc_value(*dst, v);
                }

                Instr::StructUpdate { dst, base, updates } => {
                    println!("WARNING: struct update not implemented, returning unit");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::Tuple { dst, items } => {
                    // Сгенерировать runtime-значение tuple: Vec<Value>
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

    // Pattern matching helper functions
    fn match_pattern(&self, pattern: &Pattern, value: &Value) -> Option<std::collections::HashMap<String, Value>> {
        let mut bindings = std::collections::HashMap::new();
        
        if self.pattern_matches(pattern, value, &mut bindings) {
            Some(bindings)
        } else {
            None
        }
    }

    fn pattern_matches(&self, pattern: &Pattern, value: &Value, bindings: &mut std::collections::HashMap<String, Value>) -> bool {
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
                // Match a struct
                if let Value::StructInstance { name: struct_name, fields: struct_fields } = value {
                    if struct_name != name {
                        return false;
                    }
                    
                    for field_pattern in fields {
                        match field_pattern {
                            PatternField::Shorthand { field, .. } => {
                                // Bind the field value to a variable with the same name
                                if let Some((_, v)) = struct_fields.iter().find(|(k, _)| k == field) {
                                    bindings.insert(field.clone(), v.clone());
                                } else {
                                    return false;
                                }
                            }
                            PatternField::Assign { field, pat, .. } => {
                                // Match the field value against the pattern
                                if let Some((_, v)) = struct_fields.iter().find(|(k, _)| k == field) {
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
                } else {
                    false
                }
            }
            
            Pattern::Variant { name, args, .. } => {
                // Match an enum variant
                if let Value::EnumInstance { enum_name, variant, args: variant_args } = value {
                    if variant != name {
                        return false;
                    }
                    
                    if args.len() != variant_args.len() {
                        return false;
                    }
                    
                    for (pat, val) in args.iter().zip(variant_args.iter()) {
                        if !self.pattern_matches(pat, val, bindings) {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            }
            
            // For now, other patterns just match without binding
            _ => true,
        }
    }
}
