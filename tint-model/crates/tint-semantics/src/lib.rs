pub mod attr;
pub mod checker;
pub mod errors;
pub mod prelude;
pub mod scope;
pub mod type_table;

// convenient export
pub use checker::SemanticChecker;
pub use errors::SemanticError;
