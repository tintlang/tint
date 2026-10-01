use std::fmt;

use tint_ast::Span;
use tint_runtime::vm::TintVM;

use crate::{registered_natives, FromTint, IntoArgs, NativeFn, UiSession, Value};

/// A failure loading or running Tint code.
#[derive(Debug, Clone)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for Error {}

/// A loaded Tint program: call its `fn`s from Rust.
pub struct Tint {
    vm: TintVM,
}

fn parse(source: &str) -> Result<tint_ast::Program, Error> {
    let tokens = tint_lexer::collect_tokens(&mut tint_lexer::Lexer::new(source));
    tint_parser::Parser::new(tokens)
        .parse_program()
        .map_err(|e| Error(format!("{e:?}")))
}

impl Tint {
    /// Parses `source`. Every `#[tint::export]` function in the program is
    /// callable from it.
    pub fn new(source: &str) -> Result<Tint, Error> {
        Self::with_natives(source, &registered_natives())
    }

    /// Like `new`, with an explicit set of host functions (`tint::natives![..]`).
    pub fn with_natives(source: &str, natives: &[(String, NativeFn)]) -> Result<Tint, Error> {
        let program = parse(source)?;
        let mut vm = TintVM::new();
        for (name, function) in natives {
            let function = std::rc::Rc::clone(function);
            vm.register_native(name.clone(), move |args| function(args));
        }
        vm.load_program(&program);
        Ok(Tint { vm })
    }

    /// Calls `fn name` with typed arguments and converts the result.
    pub fn call<R: FromTint>(&mut self, name: &str, args: impl IntoArgs) -> Result<R, Error> {
        let value = self.call_value(name, &args.into_args())?;
        R::from_tint(&value).map_err(|e| Error(format!("{name}: result: {e}")))
    }

    /// Calls `fn name` with raw values.
    pub fn call_value(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        self.vm
            .call_fn(name, args, Span::dummy())
            .map_err(|e| Error(e.to_string()))
    }

    /// A stateful UI session for `ui fn entry` (render it with your own backend).
    pub fn ui(source: &str, entry: &str) -> Result<UiSession, Error> {
        Self::ui_with_natives(source, entry, &registered_natives())
    }

    pub fn ui_with_natives(
        source: &str,
        entry: &str,
        natives: &[NativeFn_],
    ) -> Result<UiSession, Error> {
        UiSession::new_with_natives(source, entry, Default::default(), natives).map_err(Error)
    }
}

type NativeFn_ = (String, NativeFn);
