// ============================================
// tint-ir/src/compiler.rs — AST → SSA IR
// ============================================

use crate::builder::IrBuilder;
use crate::ir::*;

use std::collections::HashMap;
use tint_ast::PatternField;
use tint_ast::StringPart;
use tint_ast::StructInitField;
use tint_ast::{Expr, FnBody, Item, Pattern, Program, Stmt};

pub struct SsaCompiler {
    builder: IrBuilder,
}

impl SsaCompiler {
    pub fn new() -> Self {
        Self {
            builder: IrBuilder::new(),
        }
    }

    fn lower_assignment_lhs(
        &mut self,
        lhs: &Expr,
        value: ValueId,
        block: &mut Block,
        locals: &mut HashMap<String, ValueId>,
    ) {
        match lhs {
            // x = v
            Expr::Ident(name, _) => {
                self.builder.emit_store(block, name.clone(), value);
                locals.insert(name.clone(), value);
            }

            // obj.field = v
            Expr::Field { target, field, .. } => {
                let base = self.lower_expr(target, block, locals);
                self.builder
                    .emit_field_store(block, base, field.clone(), value);
            }

            // arr[i] = v
            Expr::Index { target, index, .. } => {
                let arr = self.lower_expr(target, block, locals);
                let idx = self.lower_expr(index, block, locals);
                self.builder.emit_index_store(block, arr, idx, value);
            }

            // Недопустимое LHS
            _ => {
                panic!("Invalid assignment LHS: {:?}", lhs);
            }
        }
    }
    pub fn compile_program(&mut self, prog: &Program) -> ProgramIR {
        let mut out = ProgramIR::new();

        for item in &prog.items {
            if let Item::Fn(f) = item {
                out.functions.push(self.compile_fn(f));
            }
        }

        out
    }

    fn compile_fn(&mut self, f: &tint_ast::FnDecl) -> FunctionIR {
        let mut block = self.builder.new_block();
        let mut locals = std::collections::HashMap::new();

        match &f.body {
            FnBody::Block(b) => {
                for stmt in &b.stmts {
                    self.lower_stmt(stmt, &mut block, &mut locals);
                }
            }

            FnBody::Expr(expr) => {
                let v = self.lower_expr(expr, &mut block, &mut locals);
                self.builder.emit_return(&mut block, v);
            }
        }

        FunctionIR {
            name: f.name.clone(),
            blocks: vec![block],
            locals,
            params: f.params.iter().map(|p| p.pattern.clone()).collect(),
        }
    }

    // LOWERING STATEMENT
    fn lower_stmt(
        &mut self,
        s: &Stmt,
        block: &mut Block,
        locals: &mut std::collections::HashMap<String, ValueId>,
    ) {
        match s {
            // NEW: let pattern = expr
            Stmt::Let { pattern, init, .. } => {
                let expr = match init {
                    tint_ast::LetInit::Assign(e) => e,
                    tint_ast::LetInit::Tint(e) => e,
                };

                let v = self.lower_expr(expr, block, locals);
                self.bind_pattern(pattern, v, locals, block);
            }

            Stmt::Assign { lhs, rhs, .. } => {
                let rv = self.lower_expr(rhs, block, locals);
                self.lower_assignment_lhs(lhs, rv, block, locals);
            }

            Stmt::Return(expr, _) => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_return(block, v);
            }

            Stmt::Expr(expr) => {
                self.lower_expr(expr, block, locals);
            }

            other => {
                println!("WARNING: stmt not lowered in SSA: {:?}", other);
            }
        }
    }

    // PATTERN BINDING in SS
    fn bind_pattern(
        &mut self,
        pat: &Pattern,
        value: ValueId,
        locals: &mut HashMap<String, ValueId>,
        block: &mut Block,
    ) {
        match pat {
            // x
            Pattern::Ident(name, _) => {
                locals.insert(name.clone(), value);
            }

            // _
            Pattern::Wildcard(_) => {}

            Pattern::Mut { inner, .. } => {
                self.bind_pattern(inner, value, locals, block);
            }

            // NUMBER | STRING
            Pattern::Number(_, _) => {}
            Pattern::String(_, _) => {}

            // (a, b, c)
            Pattern::Tuple(items, _) => {
                for (i, subpat) in items.iter().enumerate() {
                    let elem = self.builder.emit_tuple_extract(block, value, i);
                    self.bind_pattern(subpat, elem, locals, block);
                }
            }

            // User { id, name }
            Pattern::Struct { fields, .. } => {
                for f in fields {
                    match f {
                        PatternField::Shorthand { field, .. } => {
                            let fld = self.builder.emit_field_access(block, value, field.clone());
                            locals.insert(field.clone(), fld);
                        }

                        PatternField::Assign { field, pat, .. } => {
                            let fld = self.builder.emit_field_access(block, value, field.clone());
                            self.bind_pattern(pat, fld, locals, block);
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            }

            // { a, b: pat } — group pattern (struct-like, but without type)
            Pattern::Group { fields, .. } => {
                for f in fields {
                    match f {
                        // shorthand: { a }
                        PatternField::Shorthand { field, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            locals.insert(field.clone(), elem);
                        }

                        // assign: { a: pat }
                        PatternField::Assign { field, pat, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            self.bind_pattern(pat, elem, locals, block);
                        }

                        PatternField::Rest(_) => {}
                    }
                }
            }

            // map { x, y: pat, .. }
            Pattern::Map { fields, .. } => {
                for f in fields {
                    match f {
                        // shorthand: map { x }
                        PatternField::Shorthand { field, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            locals.insert(field.clone(), elem);
                        }

                        // field: pat
                        PatternField::Assign { field, pat, .. } => {
                            let elem = self.builder.emit_map_access(block, value, field.clone());
                            self.bind_pattern(pat, elem, locals, block);
                        }

                        // ..
                        PatternField::Rest(_) => {}
                    }
                }
            }

            // Some(x), Ok(v) — пока заглушка
            Pattern::Variant { .. } => {
                println!("TODO: variant pattern in SSA");
            }

            // typed pattern:  pat: Type
            Pattern::Typed { pat, .. } => {
                // просто рекурсивно обрабатываем внутренний паттерн
                self.bind_pattern(pat, value, locals, block);
            }
        }
    }

    // LOWERING EXPRESSION
    fn lower_expr(
        &mut self,
        e: &Expr,
        block: &mut Block,
        locals: &mut std::collections::HashMap<String, ValueId>,
    ) -> ValueId {
        match e {
            Expr::Number(n, _) => self
                .builder
                .emit_const(block, Value::Number(n.parse().unwrap())),

            Expr::String(s, _) => self.builder.emit_const(block, Value::String(s.clone())),

            Expr::Bool(v, _) => self.builder.emit_const(block, Value::Bool(*v)),

            Expr::Unit(_) => self.builder.emit_const(block, Value::Unit),

            Expr::Borrow { target, .. } => {
                // lowering: borrow(x) просто становится значением x
                self.lower_expr(target, block, locals)
            }

            Expr::Ident(name, _) => {
                if let Some(&id) = locals.get(name) {
                    id
                } else {
                    self.builder.emit_load(block, name.clone())
                }
            }

            Expr::SelfKw(span) => {
                if let Some(&id) = locals.get("self") {
                    id
                } else {
                    self.builder.emit_load(block, "self".into())
                }
            }

            Expr::Block(b, _) => {
                let mut last = self.builder.emit_const(block, Value::Unit);

                for stmt in &b.stmts {
                    self.lower_stmt(stmt, block, locals);
                    if let Stmt::Expr(e2) = stmt {
                        last = self.lower_expr(e2, block, locals);
                    }
                }

                last
            }

            // INTERPOLATED STRING → sequence of concat ops
            Expr::InterpolatedString { parts, .. } => {
                let mut acc = self.builder.emit_const(block, Value::String(String::new()));

                for part in parts {
                    let next = match part {
                        StringPart::Text(t) => {
                            self.builder.emit_const(block, Value::String(t.clone()))
                        }

                        StringPart::Expr(e2) => self.lower_expr(e2, block, locals),
                    };

                    acc = self.builder.emit_binary(block, "+".into(), acc, next);
                }

                acc
            }

            Expr::Binary {
                left, op, right, ..
            } => {
                let lv = self.lower_expr(left, block, locals);
                let rv = self.lower_expr(right, block, locals);
                self.builder.emit_binary(block, op.clone(), lv, rv)
            }

            Expr::Unary { op, expr, .. } => {
                let v = self.lower_expr(expr, block, locals);
                self.builder.emit_unary(block, op.clone(), v)
            }

            Expr::Paren(expr, _) => self.lower_expr(expr, block, locals),

            // CALL: foo(a,b)
            Expr::Call { target, args, .. } => {
                let fn_val = self.lower_expr(target, block, locals);
                let lowered = args
                    .iter()
                    .map(|a| self.lower_expr(a, block, locals))
                    .collect::<Vec<_>>();

                self.builder.emit_call(block, fn_val, lowered)
            }

            // FIELD: obj.field
            Expr::Field { target, field, .. } => {
                let base = self.lower_expr(target, block, locals);
                self.builder.emit_field_access(block, base, field.clone())
            }

            // MODULE::ITEM
            Expr::Namespace { base, item, .. } => {
                let mod_val = self.lower_expr(base, block, locals);
                self.builder
                    .emit_namespace_access(block, mod_val, item.clone())
            }

            // INDEX: arr[i]
            Expr::Index { target, index, .. } => {
                let arr = self.lower_expr(target, block, locals);
                let idx = self.lower_expr(index, block, locals);
                self.builder.emit_index(block, arr, idx)
            }

            Expr::VariantInit {
                enum_name,
                variant,
                fields,
                ..
            } => {
                // 1) Lower each field expression to SSA ValueId
                let mut lowered_fields = Vec::new();

                for f in fields {
                    let val = match f {
                        StructInitField::Assign { expr, .. } => {
                            self.lower_expr(expr, block, locals)
                        }

                        StructInitField::Tint { expr, .. } => self.lower_expr(expr, block, locals),
                    };

                    let name = match f {
                        StructInitField::Assign { name, .. } => name.clone(),
                        StructInitField::Tint { name, .. } => name.clone(),
                    };

                    lowered_fields.push((name, val));
                }

                // 2) Emit a new VariantInstance value
                self.builder.emit_variant_init(
                    block,
                    enum_name.clone(),
                    variant.clone(),
                    lowered_fields,
                )
            }

            Expr::Match {
                scrutinee, arms, ..
            } => {
                let scr = self.lower_expr(scrutinee, block, locals);

                let mut lowered_arms = Vec::new();

                for arm in arms {
                    let pat = arm.pattern.clone();

                    // A match arm introduces its own bindings (`A { x }`,
                    // `(1, b, 3)`, `x: i32`, ...) that only exist at
                    // RUNTIME -- `Instr::Match` (ir_vm.rs) inserts them by
                    // name into `self.locals` for whichever arm actually
                    // matches, right before computing that arm's result.
                    // There is no per-arm scope at COMPILE time though:
                    // this whole function lowers into one flat
                    // instruction list, and `locals` here is a single
                    // shared name -> ValueId cache used by `Expr::Ident`
                    // lowering (see above) to reuse an already-computed
                    // value instead of emitting a fresh `LoadLocal`.
                    //
                    // If a pattern's bound name happens to already be in
                    // that cache (e.g. `x` re-bound by `x: i32 => x * 2`
                    // when an OUTER `x` is already in scope), lowering the
                    // arm body resolved straight to the OUTER value's
                    // ValueId and never emitted a `LoadLocal` at all -- so
                    // the runtime binding above was computed and then
                    // never read, and the arm silently used the outer
                    // variable's (possibly stale) value instead. Shadow
                    // every name this pattern (re)binds out of the cache
                    // before lowering the body, so a reference to it is
                    // forced through `emit_load` and actually reads back
                    // the runtime binding `Instr::Match` just installed.
                    // Names are restored after the arm so sibling arms and
                    // code following the match keep seeing the outer
                    // binding.
                    let mut bound_names = Vec::new();
                    collect_pattern_names(&pat, &mut bound_names);

                    let mut shadowed = Vec::new();
                    for name in &bound_names {
                        if let Some(id) = locals.remove(name) {
                            shadowed.push((name.clone(), id));
                        }
                    }

                    // Lower the guard, if present
                    let guard_val = if let Some(ref guard_expr) = arm.guard {
                        Some(self.lower_expr(guard_expr, block, locals))
                    } else {
                        None
                    };

                    // Lower the body expression
                    let body_val = self.lower_expr(&arm.expr, block, locals);

                    for (name, id) in shadowed {
                        locals.insert(name, id);
                    }

                    lowered_arms.push((pat, guard_val, body_val));
                }

                self.builder.emit_match(block, scr, lowered_arms)
            }

            // STRUCT INIT
            Expr::StructInit { name, fields, .. } => {
                let mut fv = Vec::new();

                for f in fields {
                    match f {
                        tint_ast::StructInitField::Assign { name, expr, .. } => {
                            let v = self.lower_expr(expr, block, locals);
                            fv.push((name.clone(), v));
                        }
                        tint_ast::StructInitField::Tint { name, expr, .. } => {
                            let v = self.lower_expr(expr, block, locals);
                            fv.push((name.clone(), v));
                        }
                    }
                }

                self.builder.emit_struct_init(block, name.clone(), fv)
            }

            // STRUCT UPDATE: base { f = 10, y{expr} }
            Expr::StructUpdate { base, updates, .. } => {
                let base_val = self.lower_expr(base, block, locals);
                let mut upd_vec = Vec::new();

                for u in updates {
                    match u {
                        tint_ast::StructInitField::Assign { name, expr, .. } => {
                            let v = self.lower_expr(expr, block, locals);
                            upd_vec.push((name.clone(), v));
                        }
                        tint_ast::StructInitField::Tint { name, expr, .. } => {
                            let v = self.lower_expr(expr, block, locals);
                            upd_vec.push((name.clone(), v));
                        }
                    }
                }

                self.builder.emit_struct_update(block, base_val, upd_vec)
            }

            Expr::Array { items, .. } => {
                let vals = items
                    .iter()
                    .map(|e2| self.lower_expr(e2, block, locals))
                    .collect::<Vec<_>>();

                self.builder.emit_array(block, vals)
            }

            // TUPLE: (a,b,c)
            Expr::Tuple { items, .. } => {
                let vals = items
                    .iter()
                    .map(|e2| self.lower_expr(e2, block, locals))
                    .collect::<Vec<_>>();

                self.builder.emit_tuple(block, vals)
            }

            // TUPLE: (v.0, v.1 , v.2)
            Expr::TupleIndex { target, index, .. } => {
                let base = self.lower_expr(target, block, locals);
                self.builder.emit_tuple_extract(block, base, *index)
            }

            Expr::MapInit { entries, .. } => {
                let mut lowered = Vec::new();
                for (k, e) in entries {
                    let id = self.lower_expr(e, block, locals);
                    lowered.push((k.clone(), id));
                }
                self.builder.emit_map_init(block, lowered)
            }

            // NAMED ARG — just lower the value, name is semantic
            Expr::NamedArg { value, .. } => self.lower_expr(value, block, locals),

            // TODO λ
            Expr::Lambda { .. } => {
                println!("TODO: lambda lowering");
                self.builder.emit_const(block, Value::Unit)
            }
        }
    }
}

// Every name a pattern would bind if it matched -- used only to know which
// entries to shadow out of the compile-time `locals` cache before lowering
// a match arm's body (see the comment at `Expr::Match` above). This is
// deliberately just name collection, not real binding: unlike
// `SsaCompiler::bind_pattern` (used for `let` destructuring), it emits no
// instructions, since a match arm's actual extraction happens at runtime
// in `Instr::Match` (ir_vm.rs), keyed by these same names.
fn collect_pattern_names(pat: &Pattern, out: &mut Vec<String>) {
    match pat {
        Pattern::Ident(name, _) => out.push(name.clone()),
        Pattern::Wildcard(_) | Pattern::Number(_, _) | Pattern::String(_, _) => {}
        Pattern::Mut { inner, .. } => collect_pattern_names(inner, out),
        Pattern::Tuple(items, _) => {
            for p in items {
                collect_pattern_names(p, out);
            }
        }
        Pattern::Struct { fields, .. } | Pattern::Group { fields, .. } | Pattern::Map { fields, .. } => {
            for f in fields {
                match f {
                    PatternField::Shorthand { field, .. } => out.push(field.clone()),
                    PatternField::Assign { pat, .. } => collect_pattern_names(pat, out),
                    PatternField::Rest(_) => {}
                }
            }
        }
        Pattern::Variant { args, .. } => {
            for p in args {
                collect_pattern_names(p, out);
            }
        }
        Pattern::Typed { pat, .. } => collect_pattern_names(pat, out),
    }
}
