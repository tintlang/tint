//! Embed Tint in Rust.
//!
//! ```ignore
//! // Rust -> Tint: load a .tn file and call its functions with typed values.
//! let mut tint = tint::Tint::new(tint::tint_file!("area.tn"))?;
//! let area: f64 = tint.call("area", (3.0, 4.0))?;
//!
//! // Tint -> Rust: export a function; `.tn` calls it as `slug(...)`.
//! #[tint::export]
//! fn slug(text: String) -> String { text.to_lowercase().replace(' ', "-") }
//! ```
//!
//! Values cross the boundary by copy: numbers, strings, booleans, lists
//! (`Vec`), maps (`HashMap<String, _>`), options, tuples and structs that
//! derive `IntoTint`/`FromTint`.

mod convert;
#[cfg(feature = "host")]
mod host;

pub use convert::{Callback, FromTint, IntoArgs, IntoNative, IntoTint};
#[cfg(feature = "host")]
pub use host::{Error, Tint};
pub use tint_evaluator::errors::{EvalError, EvalResult};
pub use tint_evaluator::Value;
pub use tint_macros::{export, tint_file, FromTint, IntoTint};
#[cfg(feature = "host")]
pub use tint_runtime::ui_session::UiSession;

/// A host function: Tint arguments in, Tint value out.
pub type NativeFn = std::rc::Rc<dyn Fn(&[Value]) -> EvalResult<Value>>;

/// An error a host function can return to Tint.
pub fn native_error(message: String) -> EvalError {
    EvalError::CallError { msg: message, span: tint_ast::Span::dummy() }
}

#[doc(hidden)]
pub fn native_error_text(message: String) -> EvalError {
    native_error(message)
}

/// Every `#[tint::export]` function linked into the program (native targets).
/// On `wasm32` this is empty; pass `natives![...]` explicitly there.
pub fn registered_natives() -> Vec<(String, NativeFn)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut all: Vec<(String, NativeFn)> =
            __private::inventory::iter::<__private::Registered>().map(|entry| (entry.make)()).collect();
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}

/// `natives![area, shapes::perimeter]`: the exported functions by Rust path.
#[macro_export]
macro_rules! natives {
    ($( $($segment:ident)::+ ),* $(,)?) => {
        ::std::vec![$( $($segment)::+ ::native() ),*]
    };
}

#[cfg(not(target_arch = "wasm32"))]
mod block_on;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[doc(hidden)]
pub mod __private {
    #[cfg(not(target_arch = "wasm32"))]
    pub use inventory;
    #[cfg(target_arch = "wasm32")]
    pub use wasm_bindgen;
    #[cfg(target_arch = "wasm32")]
    pub use crate::wasm::call_native;

    pub type NativeFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::Value, String>>>>;

    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::block_on::run_async_native;
    #[cfg(target_arch = "wasm32")]
    pub use crate::wasm::call_native_async;
    #[cfg(target_arch = "wasm32")]
    pub use js_sys;

    pub struct Registered {
        pub make: fn() -> (String, crate::NativeFn),
    }
    #[cfg(not(target_arch = "wasm32"))]
    inventory::collect!(Registered);

    /// A field of a struct instance or map, for `#[derive(FromTint)]`.
    pub fn field<'a>(value: &'a crate::Value, name: &str) -> Option<&'a crate::Value> {
        match value {
            crate::Value::StructInstance { fields, .. } => {
                fields.iter().find(|(key, _)| key == name).map(|(_, item)| item)
            }
            crate::Value::Map(map) => map.get(name),
            _ => None,
        }
    }
}
