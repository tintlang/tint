//! Read tracking for UI subtree reuse (see `EvalHost::track_begin`).
//!
//! While a recording is active, every variable lookup that resolves to a
//! binding outside the recording's own scope depth is logged with the value
//! it produced. A cached subtree stays valid exactly while those variables
//! still hold those values.

use super::*;

pub(crate) struct ReadLog {
    /// Scope depth when recording started; only bindings living in frames
    /// below this index are inputs from outside.
    base: usize,
    reads: Vec<(String, Option<EvalValue>)>,
}

impl TintVM {
    pub(super) fn note_read(&mut self, name: &str, found: Option<(usize, EvalValue)>) {
        let frame = found.as_ref().map(|(frame, _)| *frame);
        let value = found.map(|(_, value)| value);
        for log in &mut self.read_logs {
            if frame.is_some_and(|frame| frame >= log.base) {
                continue; // bound inside the recorded region: not an input
            }
            if !log.reads.iter().any(|(existing, _)| existing == name) {
                log.reads.push((name.to_string(), value.clone()));
            }
        }
    }

    /// `lookup` that also feeds the active recordings.
    pub(super) fn tracked_lookup(&mut self, name: &str) -> Option<EvalValue> {
        if self.read_logs.is_empty() {
            return self.scopes.lookup(name).map(|v| Self::rt_to_eval(&v));
        }
        let found = self
            .scopes
            .lookup_ref(name)
            .map(|(frame, v)| (frame, Self::rt_to_eval(v)));
        let result = found.as_ref().map(|(_, v)| v.clone());
        self.note_read(name, found);
        result
    }

    pub(super) fn begin_tracking(&mut self) {
        self.read_logs.push(ReadLog {
            base: self.scopes.depth(),
            reads: Vec::new(),
        });
    }

    pub(super) fn end_tracking(&mut self) -> Vec<(String, Option<EvalValue>)> {
        self.read_logs
            .pop()
            .map(|log| log.reads)
            .unwrap_or_default()
    }

    pub(super) fn merge_tracking(&mut self, reads: &[(String, Option<EvalValue>)]) {
        for (name, value) in reads {
            // Same rule as `note_read`: only frames below each log's base
            // count as inputs. A merged read was an input of the child log
            // and therefore lives below the child's base; whether it is
            // also an input for the parent depends on where it resolves now.
            let frame = self.scopes.lookup_ref(name).map(|(frame, _)| frame);
            for log in &mut self.read_logs {
                if frame.is_some_and(|frame| frame >= log.base) {
                    continue;
                }
                if !log.reads.iter().any(|(existing, _)| existing == name) {
                    log.reads.push((name.clone(), value.clone()));
                }
            }
        }
    }

    pub(super) fn reads_still_hold(&self, reads: &[(String, Option<EvalValue>)]) -> bool {
        reads.iter().all(
            |(name, expected)| match (self.scopes.lookup_ref(name), expected) {
                (None, None) => true,
                (Some((_, actual)), Some(expected)) => runtime_equals(actual, expected),
                _ => false,
            },
        )
    }
}

/// Structural equality between a stored runtime value and a recorded value.
/// Anything that cannot be compared cheaply and exactly (lambdas, handles)
/// reports "different", which only costs a cache miss.
fn runtime_equals(actual: &RuntimeValue, expected: &EvalValue) -> bool {
    use EvalValue as E;
    use RuntimeValue as R;
    match (actual, expected) {
        (R::Number(a), E::Number(b)) => a == b || (a.is_nan() && b.is_nan()),
        (R::I32(a), E::I32(b)) => a == b,
        (R::I64(a), E::I64(b)) => a == b,
        (R::U32(a), E::U32(b)) => a == b,
        (R::U64(a), E::U64(b)) => a == b,
        (R::U8(a), E::U8(b)) => a == b,
        (R::F32(a), E::F32(b)) => a == b,
        (R::F64(a), E::F64(b)) => a == b,
        (R::String(a), E::String(b)) => a == b,
        (R::Bool(a), E::Bool(b)) => a == b,
        (R::Unit, E::Unit) => true,
        (R::List(a), E::List(b)) | (R::Tuple(a), E::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| runtime_equals(x, y))
        }
        (
            R::StructInstance {
                name: an,
                fields: af,
            },
            E::StructInstance {
                name: bn,
                fields: bf,
            },
        ) => {
            an == bn
                && af.len() == bf.len()
                && af
                    .iter()
                    .zip(bf)
                    .all(|((ak, av), (bk, bv))| ak == bk && runtime_equals(av, bv))
        }
        (
            R::EnumInstance {
                enum_name: ae,
                variant: av,
                args: aa,
            },
            E::EnumInstance {
                enum_name: be,
                variant: bv,
                args: ba,
            },
        ) => {
            ae == be
                && av == bv
                && aa.len() == ba.len()
                && aa.iter().zip(ba).all(|(x, y)| runtime_equals(x, y))
        }
        (R::Map(a), E::Map(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| runtime_equals(v, other)))
        }
        _ => false,
    }
}
