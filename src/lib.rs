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
    Backlink, DerivedLink, Folder, FolderChild, FolderChildKind, FolderChildStatus, FolderIssue,
    HtmlExportOptions, HtmlExportReport, Iframe, IframeBacklink, IframeTarget, Link, LinkTarget,
    Mutation, Page, PageKind, ProjectManifest, SearchResult, TextOccurrence, TextPosition,
    ValidationIssue, ValidationReport,
};

pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
mod tests;
