use std::collections::HashMap;
use std::fmt::Display;

use crate::{EvalResult, Value};

/// Rust value -> Tint value.
pub trait IntoTint {
    fn into_tint(self) -> Value;
}

/// Tint value -> Rust value. The error is a short message.
pub trait FromTint: Sized {
    fn from_tint(value: &Value) -> Result<Self, String>;
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Number(_) | Value::F64(_) | Value::F32(_) | Value::I32(_) | Value::I64(_)
        | Value::U32(_) | Value::U64(_) | Value::U8(_) => "number",
        Value::String(_) => "string",
        Value::Bool(_) => "bool",
        Value::Unit => "unit",
        Value::List(_) => "list",
        Value::Tuple(_) => "tuple",
        Value::Map(_) => "map",
        Value::StructInstance { .. } => "struct",
        Value::EnumInstance { .. } => "enum",
        Value::Callback(_) | Value::Lambda { .. } | Value::Function { .. } => "function",
        _ => "value",
    }
}

fn as_f64(value: &Value) -> Option<f64> {
    Some(match value {
        Value::Number(n) | Value::F64(n) => *n,
        Value::F32(n) => *n as f64,
        Value::I32(n) => *n as f64,
        Value::I64(n) => *n as f64,
        Value::U32(n) => *n as f64,
        Value::U64(n) => *n as f64,
        Value::U8(n) => *n as f64,
        _ => return None,
    })
}

fn expected(what: &str, value: &Value) -> String {
    format!("expected {what}, got {}", kind(value))
}

macro_rules! float_impls {
    ($($t:ty),*) => {$(
        impl IntoTint for $t {
            fn into_tint(self) -> Value { Value::Number(self as f64) }
        }
        impl FromTint for $t {
            fn from_tint(value: &Value) -> Result<Self, String> {
                as_f64(value).map(|n| n as $t).ok_or_else(|| expected("a number", value))
            }
        }
    )*};
}

// Tint's `number` is an f64: integers beyond 2^53 lose precision.
macro_rules! int_impls {
    ($($t:ty),*) => {$(
        impl IntoTint for $t {
            fn into_tint(self) -> Value { Value::Number(self as f64) }
        }
        impl FromTint for $t {
            fn from_tint(value: &Value) -> Result<Self, String> {
                let n = as_f64(value).ok_or_else(|| expected("a number", value))?;
                if n.fract() != 0.0 || n < <$t>::MIN as f64 || n > <$t>::MAX as f64 {
                    return Err(format!("{n} does not fit {}", stringify!($t)));
                }
                Ok(n as $t)
            }
        }
    )*};
}

float_impls!(f64, f32);
int_impls!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl IntoTint for bool {
    fn into_tint(self) -> Value { Value::Bool(self) }
}
impl FromTint for bool {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::Bool(b) => Ok(*b),
            other => Err(expected("a bool", other)),
        }
    }
}

impl IntoTint for String {
    fn into_tint(self) -> Value { Value::String(self) }
}
impl IntoTint for &str {
    fn into_tint(self) -> Value { Value::String(self.to_string()) }
}
impl FromTint for String {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            other => Err(expected("a string", other)),
        }
    }
}

impl IntoTint for () {
    fn into_tint(self) -> Value { Value::Unit }
}
impl FromTint for () {
    fn from_tint(_: &Value) -> Result<Self, String> { Ok(()) }
}

impl IntoTint for Value {
    fn into_tint(self) -> Value { self }
}
impl FromTint for Value {
    fn from_tint(value: &Value) -> Result<Self, String> { Ok(value.clone()) }
}

impl<T: IntoTint> IntoTint for Vec<T> {
    fn into_tint(self) -> Value {
        Value::List(self.into_iter().map(IntoTint::into_tint).collect())
    }
}
impl<T: FromTint> FromTint for Vec<T> {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::List(items) | Value::Tuple(items) => items
                .iter()
                .enumerate()
                .map(|(i, item)| T::from_tint(item).map_err(|e| format!("item {i}: {e}")))
                .collect(),
            other => Err(expected("a list", other)),
        }
    }
}

impl<T: IntoTint> IntoTint for HashMap<String, T> {
    fn into_tint(self) -> Value {
        Value::Map(self.into_iter().map(|(k, v)| (k, v.into_tint())).collect())
    }
}
impl<T: FromTint> FromTint for HashMap<String, T> {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::Map(map) => map
                .iter()
                .map(|(k, v)| T::from_tint(v).map(|v| (k.clone(), v)).map_err(|e| format!("key `{k}`: {e}")))
                .collect(),
            other => Err(expected("a map", other)),
        }
    }
}

impl<T: IntoTint> IntoTint for Option<T> {
    fn into_tint(self) -> Value {
        match self {
            Some(v) => Value::EnumInstance {
                enum_name: "Option".into(),
                variant: "Some".into(),
                args: vec![v.into_tint()],
            },
            None => Value::EnumInstance {
                enum_name: "Option".into(),
                variant: "None".into(),
                args: Vec::new(),
            },
        }
    }
}
impl<T: FromTint> FromTint for Option<T> {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::Unit => Ok(None),
            Value::EnumInstance { enum_name, variant, args } if enum_name == "Option" => {
                match (variant.as_str(), args.first()) {
                    ("Some", Some(inner)) => T::from_tint(inner).map(Some),
                    _ => Ok(None),
                }
            }
            other => T::from_tint(other).map(Some),
        }
    }
}

macro_rules! tuple_impls {
    ($(($($name:ident : $idx:tt),+)),*) => {$(
        impl<$($name: IntoTint),+> IntoTint for ($($name,)+) {
            fn into_tint(self) -> Value { Value::Tuple(vec![$(self.$idx.into_tint()),+]) }
        }
        impl<$($name: FromTint),+> FromTint for ($($name,)+) {
            fn from_tint(value: &Value) -> Result<Self, String> {
                let Value::Tuple(items) = value else { return Err(expected("a tuple", value)) };
                Ok(($($name::from_tint(items.get($idx).ok_or("tuple is too short")?)?,)+))
            }
        }
    )*};
}
tuple_impls!((A: 0, B: 1), (A: 0, B: 1, C: 2), (A: 0, B: 1, C: 2, D: 3));

/// Arguments for `Tint::call`: `()`, a tuple of values, or `Vec<Value>`.
pub trait IntoArgs {
    fn into_args(self) -> Vec<Value>;
}
impl IntoArgs for () {
    fn into_args(self) -> Vec<Value> { Vec::new() }
}
impl IntoArgs for Vec<Value> {
    fn into_args(self) -> Vec<Value> { self }
}
macro_rules! args_impls {
    ($(($($name:ident : $idx:tt),+)),*) => {$(
        impl<$($name: IntoTint),+> IntoArgs for ($($name,)+) {
            fn into_args(self) -> Vec<Value> { vec![$(self.$idx.into_tint()),+] }
        }
    )*};
}
args_impls!((A: 0), (A: 0, B: 1), (A: 0, B: 1, C: 2), (A: 0, B: 1, C: 2, D: 3), (A: 0, B: 1, C: 2, D: 3, E: 4));

/// A `Result` becomes Tint's `Result::Ok(value)` / `Result::Err(message)`, so
/// `.tn` code handles the failure itself.
impl<T: IntoTint, E: Display> IntoTint for Result<T, E> {
    fn into_tint(self) -> Value {
        match self {
            Ok(v) => Value::EnumInstance {
                enum_name: "Result".into(),
                variant: "Ok".into(),
                args: vec![v.into_tint()],
            },
            Err(e) => Value::EnumInstance {
                enum_name: "Result".into(),
                variant: "Err".into(),
                args: vec![Value::String(e.to_string())],
            },
        }
    }
}
impl<T: FromTint> FromTint for Result<T, String> {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::EnumInstance { enum_name, variant, args } if enum_name == "Result" => {
                match (variant.as_str(), args.first()) {
                    ("Ok", Some(v)) => T::from_tint(v).map(Ok),
                    ("Err", Some(Value::String(e))) => Ok(Err(e.clone())),
                    ("Err", Some(other)) => Ok(Err(other.to_string())),
                    _ => Err("malformed Result".into()),
                }
            }
            other => Err(expected("a Result", other)),
        }
    }
}

/// What an exported function returns, as a Tint value.
pub trait IntoNative {
    fn into_native(name: &str, result: Self) -> EvalResult<Value>;
}
impl<T: IntoTint> IntoNative for T {
    fn into_native(_: &str, result: Self) -> EvalResult<Value> {
        Ok(result.into_tint())
    }
}

/// A Tint function (lambda or `fn`) a host function received, or a Rust closure
/// handed to Tint. Calling it re-enters Tint: during the host call it runs
/// at once; after the host call returned (a timer, a resolved future) it runs
/// against the live session, which re-renders.
#[derive(Clone)]
pub struct Callback(tint_evaluator::Callback);

impl Callback {
    pub fn new(function: impl Fn(&[Value]) -> Result<Value, String> + 'static) -> Callback {
        Callback(tint_evaluator::Callback(std::rc::Rc::new(move |args| {
            function(args).map_err(crate::native_error)
        })))
    }

    /// Calls it with raw values.
    pub fn call_value(&self, args: &[Value]) -> Result<Value, String> {
        (self.0 .0)(args).map_err(|e| e.to_string())
    }

    /// Calls it with typed arguments and converts the result.
    pub fn call<R: FromTint>(&self, args: impl IntoArgs) -> Result<R, String> {
        let value = self.call_value(&args.into_args())?;
        R::from_tint(&value)
    }
}

impl IntoTint for Callback {
    fn into_tint(self) -> Value { Value::Callback(self.0) }
}
impl FromTint for Callback {
    fn from_tint(value: &Value) -> Result<Self, String> {
        match value {
            Value::Callback(cb) => Ok(Callback(cb.clone())),
            other => Err(expected("a function", other)),
        }
    }
}
