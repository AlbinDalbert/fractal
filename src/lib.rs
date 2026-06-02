#[cfg(feature = "cli")]
pub mod cli;
mod document;
mod graph;
mod index;
mod io;
mod ops;
pub mod project;
mod types;
mod validation;

use std::error::Error;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[cfg(test)]
mod tests;
