use crate::ir::*;

pub struct IrVM {
    pub program: ProgramIR,
    locals: std::collections::HashMap<String, Value>,
    values: Vec<Value>, // SSA value table
}

impl IrVM {
    pub fn new(program: ProgramIR) -> Self {
        Self {
            program,
            locals: Default::default(),
            values: vec![],
        }
    }

    pub fn run(&mut self, entry: &str) -> Value {
        let index = self
            .program
            .functions
            .iter()
            .position(|f| f.name == entry)
            .expect("function not found");

        // теперь нет активного заимствования
        let func = self.program.functions[index].clone();

        self.run_function(&func)
    }

    fn alloc_value(&mut self, id: ValueId, v: Value) {
        if id as usize >= self.values.len() {
            self.values.resize((id + 1) as usize, Value::Unit);
        }
        self.values[id as usize] = v;
    }

    fn get_value(&self, id: ValueId) -> Value {
        self.values[id as usize].clone()
    }

    fn run_function(&mut self, func: &FunctionIR) -> Value {
        // Run only block 0 for now
        let block = &func.blocks[0];

        for instr in &block.instrs {
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
                    let l = self.get_value(*lhs).unwrap_number();
                    let r = self.get_value(*rhs).unwrap_number();

                    let out = match op.as_str() {
                        "+" => l + r,
                        "-" => l - r,
                        "*" => l * r,
                        "/" => l / r,
                        _ => panic!("Unknown binary op {}", op),
                    };

                    self.alloc_value(*dst, Value::Number(out));
                }

                Instr::Call { dst, func, args } => {
                    println!("WARNING: Call not implemented, returning unit");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::FieldAccess { dst, .. } => {
                    println!("WARNING: field access not implemented");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::NamespaceAccess { dst, .. } => {
                    println!("WARNING: namespace access not implemented");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::Index { dst, .. } => {
                    println!("WARNING: index access not implemented");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::FieldStore { .. } => {
                    println!("WARNING: field store not implemented");
                }

                Instr::IndexStore { .. } => {
                    println!("WARNING: index store not implemented");
                }

                Instr::Match { dst, .. } => {
                    println!("WARNING: IR match not implemented");
                    self.alloc_value(*dst, Value::Unit);
                }

                Instr::StructInit { dst, .. } => {
                    println!("WARNING: struct init not implemented");
                    self.alloc_value(*dst, Value::Unit);
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
                    return self.get_value(*id);
                }
            }
        }

        Value::Unit
    }
}

