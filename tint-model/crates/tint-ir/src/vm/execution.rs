use super::*;

impl<'a> IrVM<'a> {
    pub(super) fn exec_instr(&mut self, instr: &Instr) -> Option<Value> {
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

                        (Value::StructInstance { .. }, "+", Value::StructInstance { .. }) => {
                            Self::vec2_binary(&l, &r, "+")
                        }
                        (Value::StructInstance { .. }, "-", Value::StructInstance { .. }) => {
                            Self::vec2_binary(&l, &r, "-")
                        }
                        (Value::StructInstance { .. }, "*", Value::Number(scalar)) => {
                            Self::vec2_scale(&l, *scalar)
                        }
                        (Value::Number(scalar), "*", Value::StructInstance { .. }) => {
                            Self::vec2_scale(&r, *scalar)
                        }

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

            Instr::Call {
                dst,
                func,
                args,
                method,
            } => {
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

                let result = if let Some((receiver_id, method_name)) = method {
                    let receiver = self.get_value(*receiver_id);
                    match self.method_call.as_deref_mut() {
                        Some(call_method) => {
                            match call_method(receiver, method_name, &arg_values) {
                                Some((updated_receiver, result)) => {
                                    self.write_back(*receiver_id, updated_receiver);
                                    result
                                }
                                None => Value::Unit,
                            }
                        }
                        None => {
                            println!(
                                "WARNING: method call `{}` has no runtime method dispatcher",
                                method_name
                            );
                            Value::Unit
                        }
                    }
                } else {
                    match callee_name {
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
                                other => {
                                    panic!("match guard must evaluate to a bool, got {:?}", other)
                                }
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
}
