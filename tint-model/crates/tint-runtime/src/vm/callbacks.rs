// Callbacks between Tint and host functions.
//
// A Tint lambda or `fn` passed to a native function arrives there as a
// `Value::Callback`. Calling it while the native runs re-enters this VM
// (`ACTIVE` points at it for the duration of the native call); calling it
// later -- from a timer, a resolved promise -- goes through the host's
// deferred runner, which runs it against the session and re-renders.

use std::cell::{Cell, RefCell};

use super::*;
use tint_evaluator::errors::EvalError;
use tint_evaluator::value::Callback;

/// Runs a callback that is invoked after the native call returned.
pub type DeferredRunner = Rc<dyn Fn(EvalValue, Vec<EvalValue>)>;

thread_local! {
    static ACTIVE: Cell<*mut TintVM> = const { Cell::new(std::ptr::null_mut()) };
    static DEFERRED: RefCell<Option<DeferredRunner>> = const { RefCell::new(None) };
}

/// Installs (or clears) the host's deferred runner for this thread.
pub fn set_deferred_runner(runner: Option<DeferredRunner>) {
    DEFERRED.with(|slot| *slot.borrow_mut() = runner);
}

fn is_callable(value: &EvalValue) -> bool {
    matches!(value, EvalValue::Lambda { .. } | EvalValue::Function { .. })
}

fn callback_for(function: EvalValue) -> EvalValue {
    EvalValue::Callback(Callback(Rc::new(move |args: &[EvalValue]| {
        let vm = ACTIVE.with(Cell::get);
        if !vm.is_null() {
            // SAFETY: `ACTIVE` is only set by `call_native` while that VM is
            // blocked inside the native call that is now calling back; the
            // native holds no reference into the VM.
            let vm = unsafe { &mut *vm };
            let function = function.clone();
            return std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                vm.call_value(function, args, Span::dummy())
            }))
            .map_err(|payload| EvalError::CallError {
                msg: panic_text(payload),
                span: Span::dummy(),
            });
        }
        let runner = DEFERRED.with(|slot| slot.borrow().clone());
        match runner {
            Some(run) => {
                run(function.clone(), args.to_vec());
                Ok(EvalValue::Unit)
            }
            None => Err(EvalError::CallError {
                msg: "callback called after its native function returned, and the host has no event loop"
                    .to_string(),
                span: Span::dummy(),
            }),
        }
    })))
}

fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "callback failed".to_string())
}

impl TintVM {
    /// True when `name` lives in the outermost scope (`state`, constants): a
    /// lambda sees those live instead of as the copy it captured, so a callback
    /// can update state.
    pub(crate) fn is_root_binding(&self, name: &str) -> bool {
        matches!(self.scopes.lookup_ref(name), Some((0, _)))
    }

    /// Calls the registered native `name`, if there is one. Lambdas among the
    /// arguments are handed over as callbacks.
    pub(crate) fn call_native(&mut self, name: &str, args: &[EvalValue]) -> Option<EvalResult<EvalValue>> {
        let natives = Rc::clone(&self.native_fns);
        let natives = natives.borrow();
        let function = natives.get(name)?;

        let adapted: Vec<EvalValue>;
        let args = if args.iter().any(is_callable) {
            adapted = args
                .iter()
                .map(|arg| if is_callable(arg) { callback_for(arg.clone()) } else { arg.clone() })
                .collect();
            &adapted[..]
        } else {
            args
        };

        let previous = ACTIVE.with(|slot| slot.replace(self as *mut TintVM));
        let result = function(args);
        ACTIVE.with(|slot| slot.set(previous));
        Some(result)
    }
}
