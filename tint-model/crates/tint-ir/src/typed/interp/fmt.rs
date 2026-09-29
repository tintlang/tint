//! Three renderings of a value, always driven by its static type:
//! `display` (what `"{x}"` prints), `debug` (what `print` writes) and
//! `render` (the canonical form the conformance suite compares).

use super::*;

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Display,
    Debug,
    Render,
}

pub fn display(module: &Module, ty: TyId, v: &Val) -> String {
    let mut out = String::new();
    write(module, ty, v, Style::Display, &mut out);
    out
}

pub fn debug(module: &Module, ty: TyId, v: &Val) -> String {
    let mut out = String::new();
    write(module, ty, v, Style::Debug, &mut out);
    out
}

pub fn render(module: &Module, ty: TyId, v: &Val) -> String {
    let mut out = String::new();
    write(module, ty, v, Style::Render, &mut out);
    out
}

fn number(kind: NumKind, v: &Val, style: Style) -> String {
    let (text, suffix) = match (kind, v) {
        (NumKind::F32, Val::Float(f)) => (format!("{}", *f as f32), "f32"),
        (NumKind::F64, Val::Float(f)) => (format!("{f}"), if style == Style::Render { "" } else { "f64" }),
        (NumKind::Num, Val::Float(f)) => (format!("{f}"), ""),
        (NumKind::U64, Val::Int(i)) => (format!("{}", *i as u64), "u64"),
        (NumKind::I32, Val::Int(i)) => (format!("{i}"), "i32"),
        (NumKind::I64, Val::Int(i)) => (format!("{i}"), "i64"),
        (NumKind::U8, Val::Int(i)) => (format!("{i}"), "u8"),
        (NumKind::U32, Val::Int(i)) => (format!("{i}"), "u32"),
        (_, other) => (format!("{other:?}"), ""),
    };
    if style == Style::Display {
        text
    } else {
        format!("{text}{suffix}")
    }
}

fn write(module: &Module, ty: TyId, v: &Val, style: Style, out: &mut String) {
    let types = &module.types;
    match (types.kind(ty), v) {
        (_, Val::Undef) => out.push_str("<undef>"),
        (_, Val::Unit) => out.push_str(if style == Style::Debug { "unit" } else { "()" }),
        (_, Val::Bool(b)) => out.push_str(&b.to_string()),
        (TyKind::Num(kind), _) => out.push_str(&number(*kind, v, style)),
        (_, Val::Str(s)) => {
            if style == Style::Display {
                out.push_str(s);
            } else {
                out.push_str(&format!("{:?}", s.as_ref()));
            }
        }
        (TyKind::List(item), Val::List(items)) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(module, *item, x, style, out);
            }
            out.push(']');
        }
        (TyKind::Tuple(tys), Val::Tuple(items)) => {
            out.push('(');
            for (i, (t, x)) in tys.iter().zip(items.iter()).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write(module, *t, x, style, out);
            }
            out.push(')');
        }
        (TyKind::Map(item), Val::Map(map)) => {
            let (open, close) = if style == Style::Render { ("{", "}") } else { ("map { ", " }") };
            out.push_str(open);
            for (i, (k, x)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                write(module, *item, x, style, out);
            }
            out.push_str(close);
        }
        (TyKind::Adt(id), Val::Adt(a)) => {
            let def = types.adt(*id);
            match &def.body {
                AdtBody::Struct(fields) => {
                    out.push_str(&def.base);
                    out.push_str(" { ");
                    for (i, (f, x)) in fields.iter().zip(a.fields.iter()).enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(&f.name);
                        out.push_str(": ");
                        write(module, f.ty, x, style, out);
                    }
                    out.push_str(" }");
                }
                AdtBody::Enum(variants) => {
                    let variant = &variants[a.tag as usize];
                    out.push_str(&def.base);
                    out.push_str("::");
                    out.push_str(&variant.name);
                    // The conformance form leaves the parentheses off unit variants.
                    if style != Style::Render || !variant.fields.is_empty() {
                        out.push('(');
                        for (i, (f, x)) in variant.fields.iter().zip(a.fields.iter()).enumerate() {
                            if i > 0 {
                                out.push_str(", ");
                            }
                            write(module, f.ty, x, style, out);
                        }
                        out.push(')');
                    }
                }
            }
        }
        (_, Val::Closure(c)) => {
            let func = module.func(c.func);
            if style == Style::Render {
                out.push_str("<function>");
            } else if func.kind == FuncKind::Lambda {
                out.push_str("<lambda>");
            } else {
                out.push_str(&format!("<fn {}>", func.name));
            }
        }
        (_, other) => out.push_str(&format!("{other:?}")),
    }
}
