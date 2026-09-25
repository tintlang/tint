pub mod checker;
pub mod scope;
pub mod type_table;
pub mod errors;
pub mod prelude;
pub mod attr;

// convenient export
pub use checker::SemanticChecker;
pub use errors::SemanticError;
