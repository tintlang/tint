use std::fmt;

use super::Value;

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::String(s) => write!(f, "{}", s),
            Value::Unit => write!(f, "()"),

            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }

                        Value::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }

                        Value::StructInstance { name, fields } => {
                write!(f, "{} {{ ", name)?;
                let mut first = true;
                for (k, v) in fields {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }

                        Value::EnumInstance { enum_name, variant, args } => {
                write!(f, "{}::{}(", enum_name, variant)?;
                for (i, v) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }

            Value::Map(map) => {
                write!(f, "map {{ ")?;
                let mut first = true;
                for (k, v) in map {
                    if !first { write!(f, ", ")?; }
                    first = false;
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, " }}")
            }

            Value::Lambda { .. } => write!(f, "<lambda>"),
            Value::Function { name, .. } => write!(f, "<fn {}>", name),
            Value::HostFunction(_) => write!(f, "<host-fn>"),
            Value::Namespace { name, .. } => write!(f, "<namespace {}>", name),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{:?}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Unit => write!(f, "unit"),

            Value::Tuple(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", item)?;
                }
                write!(f, ")")
            }

            Value::StructInstance { name, fields } => {
                write!(f, "{} {{ ", name)?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    write!(f, "{}: {:?}", k, v)?;
                    if i + 1 < fields.len() { write!(f, ", ")?; }
                }
                write!(f, " }}")
            }

            Value::EnumInstance { enum_name, variant, args } => {
                write!(f, "{}::{}(", enum_name, variant)?;
                for (i, v) in args.iter().enumerate() {
                    write!(f, "{:?}", v)?;
                    if i + 1 < args.len() { write!(f, ", ")?; }
                }
                write!(f, ")")
            }

            Value::List(list) => write!(f, "{:?}", list),

            Value::Map(map) => {
                write!(f, "map {{ ")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    write!(f, "{}: {:?}", k, v)?;
                    if i + 1 < map.len() { write!(f, ", ")?; }
                }
                write!(f, " }}")
            }

            Value::Lambda { .. } =>
                write!(f, "<lambda>"),

            Value::Function { name, .. } =>
                write!(f, "<fn {}>", name),

            Value::HostFunction(_) =>
                write!(f, "<host-fn>"),

            Value::Namespace { name, .. } =>
                write!(f, "<namespace {}>", name),
        }
    }
}
