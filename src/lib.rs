#[cfg(feature = "cli")]
pub mod cli;
pub mod document;
mod error;
pub mod graph;
pub mod index;
mod io;
pub mod ops;
pub mod project;
mod types;
pub mod validation;

pub use error::{FractalError, FractalErrorCode};

pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
mod tests;
