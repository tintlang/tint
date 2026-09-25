// ui/reconciler.rs

use super::tree::UiTree;

pub struct UiReconciler;

impl UiReconciler {
    pub fn reconcile(_old: &UiTree, _new: &UiTree) {
        // TODO: minimal diffing (React-like)
    }
}
