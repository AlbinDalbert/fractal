//! Fractal makes operations on linked HTML documents cheap, reliable, and composable.

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
