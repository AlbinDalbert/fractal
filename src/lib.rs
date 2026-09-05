//! A project engine for native Fractal documents.
//!
//! Fractal creates, validates, repairs, mutates, searches, links, and exports
//! native Fractal document projects. Other files may coexist in project
//! directories, but Fractal does not interpret or manage them.
//!
//! Project format 2 is the only supported format. Opening a project builds an
//! in-memory native document catalog and native link index without writing
//! generated state.
//!
//! # Example
//!
//! ```no_run
//! use fractal::{Project, Result};
//!
//! fn main() -> Result<()> {
//!     let mut project = Project::init("field-notes", "Field notes")?;
//!     project.create_folder("", "Trips")?;
//!     project.create_page_at("trips/stockholm.fractal.html", "Stockholm")?;
//!
//!     let page = project.page("trips/stockholm")?;
//!     println!("{}", page.content_hash);
//!
//!     for result in project.search("Stockholm") {
//!         println!("{}", result.path);
//!     }
//!
//!     Ok(())
//! }
//! ```

#[cfg(feature = "cli")]
/// Command-line argument parsing and command dispatch.
pub mod cli;
mod document;
mod error;
mod project;
mod types;

pub use error::{FractalError, FractalErrorCode};
pub use project::Project;
pub use types::{
    Backlink, DerivedLink, Folder, FolderChild, FolderChildKind, FolderChildStatus,
    FolderHtmlExportOptions, FolderHtmlExportReport, FolderIssue, HealthIssue, HealthIssueCode,
    HtmlExportOptions, HtmlExportReport, Link, LinkTarget, MutationKind, MutationReceipt,
    NativeDocumentParts, NativePageDraft, OperationFailure, OperationWarning, OperationWarningCode,
    Page, ProjectChange, ProjectEntryKind, ProjectInspection, ProjectManifest, ProjectPath,
    ProposedRepair, RecoveryReport, RecoveryTransaction, RecoveryTransactionStatus, RepairReport,
    SearchResult, SkippedExportPage, TextOccurrence, TextPosition, ValidationIssue,
    ValidationReport,
};

/// The result type returned by Fractal operations.
pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
pub(crate) use project::{inject_transaction_fault, TransactionFaultPoint};

#[cfg(test)]
mod tests;
