pub mod attr;
pub mod checker;
pub mod errors;
pub mod prelude;
pub mod scope;
pub mod type_table;

// convenient export
pub use checker::{Reference, SemanticModel, Symbol, SymbolKind, TypedExpr};
pub use checker::{SemanticChecker, SemanticContext};
pub use errors::SemanticError;
pub use type_table::Type;
