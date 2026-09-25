// Compiler module: lowers AST to SSA IR
pub mod compiler;
pub mod lower_expr;
pub mod lower_stmt;
pub mod bind_pattern;

pub use compiler::{SsaCompiler};
pub use lower_expr::lower_expr;
pub use lower_stmt::lower_stmt;
pub use bind_pattern::{bind_pattern, lower_assignment_lhs};
