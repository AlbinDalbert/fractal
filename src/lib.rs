#[cfg(feature = "cli")]
pub mod cli;
mod document;
mod error;
mod graph;
mod index;
mod io;
mod ops;
pub mod project;
mod types;
mod validation;

pub use error::{FractalError, FractalErrorCode};

pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
mod tests;
