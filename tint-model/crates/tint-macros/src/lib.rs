//! `#[tint::export]`, `#[derive(IntoTint, FromTint)]` and `tint_file!`.
//! Use them through the `tint` crate, which re-exports them.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, FnArg, ItemFn, LitStr};

/// Makes a Rust function callable from Tint as `name(...)`.
///
/// ```ignore
/// #[tint::export]
/// fn area(w: f64, h: f64) -> f64 { w * h }
///
/// #[tint::export(name = "parse")]
/// fn parse_number(text: String) -> Result<f64, String> { ... }
/// ```
///
/// Parameters implement `FromTint`, the result `IntoTint`. A `Result<T, E>`
/// (E: Display) arrives in Tint as `Result::Ok(..)` / `Result::Err(message)`. The function stays an
/// ordinary Rust function. A module with the same name holds `native()`, which
/// `tint::natives![area]` and `Tint::with_natives` use; on native targets the
/// function is also registered automatically (see `tint::registered_natives`).
#[proc_macro_attribute]
pub fn export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let mut tint_name: Option<String> = None;
    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("name") {
                tint_name = Some(meta.value()?.parse::<LitStr>()?.value());
                Ok(())
            } else {
                Err(meta.error("expected `name = \"...\"`"))
            }
        });
        parse_macro_input!(attr with parser);
    }

    let is_async = function.sig.asyncness.is_some();
    if !function.sig.generics.params.is_empty() {
        return syn::Error::new_spanned(&function.sig.generics, "exported functions cannot be generic")
            .to_compile_error()
            .into();
    }

    let ident = function.sig.ident.clone();
    let name = tint_name.unwrap_or_else(|| ident.to_string());
    let mut types = Vec::new();
    for input in &function.sig.inputs {
        match input {
            FnArg::Typed(typed) => types.push((*typed.ty).clone()),
            FnArg::Receiver(receiver) => {
                return syn::Error::new_spanned(receiver, "exported functions cannot take `self`")
                    .to_compile_error()
                    .into();
            }
        }
    }
    let js_name = format!("__tint_{name}");
    let count = types.len();
    let vars: Vec<_> = (0..count).map(|i| format_ident!("arg{}", i)).collect();
    let conversions = types.iter().zip(&vars).enumerate().map(|(i, (ty, var))| {
        quote! {
            let #var = <#ty as ::tint::FromTint>::from_tint(&args[#i]).map_err(|error| {
                ::tint::native_error(format!("{}: argument {}: {}", #name, #i + 1, error))
            })?;
        }
    });

    if is_async {
        return export_async(&function, &ident, &name, &js_name, &types, &vars);
    }

    quote! {
        #function

        #[allow(non_snake_case, dead_code, unused_imports)]
        pub mod #ident {
            use super::*;

            /// The Tint name and host function for this export.
            pub fn native() -> (::std::string::String, ::tint::NativeFn) {
                let function: ::tint::NativeFn = ::std::rc::Rc::new(
                    |args: &[::tint::Value]| -> ::tint::EvalResult<::tint::Value> {
                        if args.len() != #count {
                            return ::std::result::Result::Err(::tint::native_error(format!(
                                "{}: expected {} argument(s), got {}", #name, #count, args.len()
                            )));
                        }
                        #(#conversions)*
                        ::tint::IntoNative::into_native(#name, super::#ident(#(#vars),*))
                    },
                );
                (::std::string::String::from(#name), function)
            }

            #[cfg(not(target_arch = "wasm32"))]
            ::tint::__private::inventory::submit! {
                ::tint::__private::Registered { make: native }
            }

            // In the browser build (`tint build` with `rs::`) every export is a
            // `__tint_<name>(args)` function of the wasm module.
            #[cfg(target_arch = "wasm32")]
            #[::tint::__private::wasm_bindgen::prelude::wasm_bindgen(
                wasm_bindgen = ::tint::__private::wasm_bindgen,
                js_name = #js_name
            )]
            pub fn __tint_wasm_export(
                args: ::tint::__private::wasm_bindgen::JsValue,
            ) -> ::std::result::Result<
                ::tint::__private::wasm_bindgen::JsValue,
                ::tint::__private::wasm_bindgen::JsValue,
            > {
                ::tint::__private::call_native(native(), args)
            }
        }
    }
    .into()
}

/// `async fn` exports. They must return `Result<T, E: Display>`. From Tint the
/// call takes a trailing callback: `name(a, b, |r| ...)` gets `Result::Ok/Err`
/// (with `await` sugar: `let r = await name(a, b);`). In the browser the wasm
/// export returns a Promise.
fn export_async(
    function: &ItemFn,
    ident: &syn::Ident,
    name: &str,
    js_name: &str,
    types: &[syn::Type],
    vars: &[syn::Ident],
) -> TokenStream {
    let count = types.len();
    let conversions = types.iter().zip(vars).enumerate().map(|(i, (ty, var))| {
        quote! {
            let #var = <#ty as ::tint::FromTint>::from_tint(&values[#i]).map_err(|error| {
                ::std::format!("{}: argument {}: {}", #name, #i + 1, error)
            })?;
        }
    });
    quote! {
        #function

        #[allow(non_snake_case, dead_code, unused_imports)]
        pub mod #ident {
            use super::*;

            pub fn native_future(values: ::std::vec::Vec<::tint::Value>) -> ::tint::__private::NativeFuture {
                ::std::boxed::Box::pin(async move {
                    if values.len() != #count {
                        return ::std::result::Result::Err(::std::format!(
                            "{}: expected {} argument(s), got {}", #name, #count, values.len()
                        ));
                    }
                    #(#conversions)*
                    match super::#ident(#(#vars),*).await {
                        ::std::result::Result::Ok(value) => ::std::result::Result::Ok(::tint::IntoTint::into_tint(value)),
                        ::std::result::Result::Err(error) => ::std::result::Result::Err(::std::string::ToString::to_string(&error)),
                    }
                })
            }

            #[cfg(not(target_arch = "wasm32"))]
            pub fn native() -> (::std::string::String, ::tint::NativeFn) {
                let function: ::tint::NativeFn = ::std::rc::Rc::new(
                    |args: &[::tint::Value]| -> ::tint::EvalResult<::tint::Value> {
                        ::tint::__private::run_async_native(#name, #count, args, native_future)
                    },
                );
                (::std::string::String::from(#name), function)
            }

            #[cfg(not(target_arch = "wasm32"))]
            ::tint::__private::inventory::submit! {
                ::tint::__private::Registered { make: native }
            }

            #[cfg(target_arch = "wasm32")]
            #[::tint::__private::wasm_bindgen::prelude::wasm_bindgen(
                wasm_bindgen = ::tint::__private::wasm_bindgen,
                js_name = #js_name
            )]
            pub fn __tint_wasm_export(
                args: ::tint::__private::wasm_bindgen::JsValue,
            ) -> ::tint::__private::js_sys::Promise {
                ::tint::__private::call_native_async(#name, args, native_future)
            }
        }
    }
    .into()
}

/// `#[derive(IntoTint)]` for structs with named fields: becomes a Tint struct
/// instance of the same name.
#[proc_macro_derive(IntoTint)]
pub fn derive_into_tint(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fields = match named_fields(&input) {
        Ok(fields) => fields,
        Err(error) => return error.to_compile_error().into(),
    };
    let entries = fields.iter().map(|field| {
        let key = field.to_string();
        quote! { (::std::string::String::from(#key), ::tint::IntoTint::into_tint(self.#field)) }
    });
    let label = name.to_string();
    quote! {
        impl ::tint::IntoTint for #name {
            fn into_tint(self) -> ::tint::Value {
                ::tint::Value::StructInstance {
                    name: ::std::string::String::from(#label),
                    fields: ::std::vec![#(#entries),*],
                }
            }
        }
    }
    .into()
}

/// `#[derive(FromTint)]` for structs with named fields: reads a Tint struct
/// instance or map by field name.
#[proc_macro_derive(FromTint)]
pub fn derive_from_tint(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fields = match named_fields(&input) {
        Ok(fields) => fields,
        Err(error) => return error.to_compile_error().into(),
    };
    let reads = fields.iter().map(|field| {
        let key = field.to_string();
        quote! {
            #field: ::tint::FromTint::from_tint(
                ::tint::__private::field(value, #key).ok_or_else(|| {
                    ::std::format!("missing field `{}`", #key)
                })?,
            ).map_err(|error| ::std::format!("field `{}`: {}", #key, error))?,
        }
    });
    quote! {
        impl ::tint::FromTint for #name {
            fn from_tint(value: &::tint::Value) -> ::std::result::Result<Self, ::std::string::String> {
                ::std::result::Result::Ok(Self { #(#reads)* })
            }
        }
    }
    .into()
}

fn named_fields(input: &DeriveInput) -> syn::Result<Vec<syn::Ident>> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&input.generics, "generic structs are not supported"));
    }
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => Ok(named.named.iter().filter_map(|f| f.ident.clone()).collect()),
            _ => Err(syn::Error::new_spanned(&input.ident, "only structs with named fields are supported")),
        },
        _ => Err(syn::Error::new_spanned(&input.ident, "only structs are supported")),
    }
}

/// `tint::tint_file!("app.tn")`: the file's text as a `&'static str`, parsed
/// at compile time so a syntax error fails `cargo build`. The path is
/// relative to the crate root (`CARGO_MANIFEST_DIR`).
#[proc_macro]
pub fn tint_file(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr);
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let full = std::path::Path::new(&root).join(path.value());
    let text = match std::fs::read_to_string(&full) {
        Ok(text) => text,
        Err(error) => {
            return syn::Error::new(path.span(), format!("cannot read {}: {}", full.display(), error))
                .to_compile_error()
                .into();
        }
    };
    let tokens = tint_lexer::collect_tokens(&mut tint_lexer::Lexer::new(&text));
    if let Err(error) = tint_parser::Parser::new(tokens).parse_program() {
        return syn::Error::new(path.span(), format!("{}: {:?}", full.display(), error))
            .to_compile_error()
            .into();
    }
    let full = full.to_string_lossy().into_owned();
    quote! { include_str!(#full) }.into()
}
