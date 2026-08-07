//! Fractal makes operations on linked HTML documents cheap, reliable, and composable.

#[cfg(feature = "cli")]
pub mod cli;
mod document;
mod error;
mod project;
mod types;

pub use error::{FractalError, FractalErrorCode};
pub use project::Project;
pub use types::{
    Backlink, Link, LinkCandidate, LinkSuggestion, LinkTarget, MatchKind, Mutation, Page,
    ProjectManifest, SearchResult, ValidationIssue, ValidationReport,
};

pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
mod tests;
