pub mod checker;
pub mod scope;
pub mod type_table;
pub mod errors;
pub mod prelude;

// convenient export
pub use checker::SemanticChecker;
pub use errors::SemanticError;
