// rune-evaluator/eval_fn.rs

use crate::{value::Value, EvalHost};
use rune_ast::{FnDecl, Span};

pub fn eval_user_fn<H: EvalHost>(
    host: &mut H,
    f: &FnDecl,
    args: &[Value],
    _span: Span,
) -> Value {
    // Открываем новый scope
    host.push_scope();

    // Связываем параметры
    for (param, arg) in f.params.iter().zip(args.iter()) {
        host.define_var(&param.name, arg.clone());
    }

    // Выполняем тело функции
    let result = host.eval_block(&f.body);

    // Закрываем scope
    host.pop_scope();

    result
}
