pub mod tree;
pub mod builder;
pub mod reconciler;
pub mod events;
pub mod layout;
pub mod render;

use rune_ast::*;
use crate::ui::tree::UiNodeId;


pub struct UiRuntime {
    pub root: Option<UiNodeId>,
}
impl UiRuntime {
    pub fn mount(&mut self, ui_fn: &UiFnDecl) {
        // Здесь происходит:
        // 1) выполнение state/logic внутри ui fn
        // 2) создание корневой ноды UI tree
        // 3) начало рендера
        let root = &ui_fn.body; // UiNode
        // TODO: builder.build(root)
    }
}
