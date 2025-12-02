// rune-evaluator/lib.rs

pub mod value;
pub mod env;
pub mod call;
pub mod eval_expr;
pub mod eval_stmt;
pub mod eval_fn;
pub mod eval_block;
pub mod errors;
pub mod eval_host;
pub mod host_vm;


pub use eval_host::EvalHost;
pub use value::Value;
pub use env::Env;
