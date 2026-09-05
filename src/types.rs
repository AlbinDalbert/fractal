use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

use crate::error::FractalErrorCode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    #[serde(alias = "project_name")]
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub path: String,
    /// SHA-256 of the exact UTF-8 source bytes, prefixed with `sha256:`.
    pub content_hash: String,
    pub kind: PageKind,
    pub title: Option<String>,
    pub text: String,
    pub links: Vec<Link>,
    pub iframes: Vec<Iframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Folder {
    /// Path below `pages/`. The pages root is represented by an empty string.
    pub path: String,
    pub title: String,
    /// The stored explicit order. `None` means the default folder-first order.
    pub order: Option<Vec<String>>,
    /// Children in effective display order, including missing ordered children.
    pub children: Vec<FolderChild>,
    pub issues: Vec<FolderIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderChild {
    pub name: String,
    pub kind: FolderChildKind,
    pub status: FolderChildStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FolderChildKind {
    Folder,
    Native,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FolderChildStatus {
    Present,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderIssue {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Native,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeDocumentParts {
    pub title: String,
    pub title_hash: String,
    pub content_html: String,
    pub content_hash: String,
    pub style_css: String,
    pub style_hash: String,
    pub metadata_html: String,
    pub metadata_hash: String,
    pub head_links_html: String,
    pub head_links_hash: String,
    /// SHA-256 of the exact complete source bytes.
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub href: String,
    pub text: String,
    pub target: LinkTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LinkTarget {
    Internal(String),
    InternalFile(String),
    External(String),
    Fragment(String),
    Broken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Iframe {
    pub src: Option<String>,
    pub title: Option<String>,
    pub sandbox: Option<String>,
    pub target: IframeTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IframeBacklink {
    pub page: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum IframeTarget {
    Internal(String),
    InternalFile(String),
    External(String),
    Inline,
    Missing,
    Broken(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Backlink {
    pub page: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub path: String,
    pub title: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedLink {
    pub text: String,
    pub target: String,
    pub occurrence: TextOccurrence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextOccurrence {
    pub start: TextPosition,
    pub end: TextPosition,
}

/// A DOM text-node ordinal and UTF-16 code-unit offset within it.
///
/// Text nodes are numbered in document order below the native document root,
/// or below `body` for raw HTML. Counting includes text nodes that Fractal does
/// not derive links inside, so a DOM `TreeWalker` can resolve the ordinal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextPosition {
    pub text_node: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationIssue {
    pub path: Option<String>,
    pub message: String,
}

/// A validated UTF-8 path relative to the project root.
///
/// Serialized paths always use `/` separators. The root manifest is
/// `fractal.json`; page and folder paths begin with `pages/`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct ProjectPath(String);

impl ProjectPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
}

impl fmt::Display for ProjectPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProjectPath {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;
        let has_windows_drive_prefix = matches!(
            path.as_bytes(),
            [drive, b':', ..] if drive.is_ascii_alphabetic()
        );
        if has_windows_drive_prefix
            || path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(serde::de::Error::custom(
                "project path must be a slash-separated relative path",
            ));
        }
        Ok(Self(path))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ProjectChange {
    Created {
        path: ProjectPath,
        entry: ProjectEntryKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        after_hash: Option<String>,
    },
    Updated {
        path: ProjectPath,
        before_hash: String,
        after_hash: String,
    },
    Moved {
        from: ProjectPath,
        to: ProjectPath,
        entry: ProjectEntryKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        before_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        after_hash: Option<String>,
    },
    Deleted {
        path: ProjectPath,
        entry: ProjectEntryKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        before_hash: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    CreatePage,
    RecreatePage,
    WriteRawPage,
    SetPageContent,
    SetPageStyle,
    SetPageMetadata,
    SetPageHeadLinks,
    RepairPageStructure,
    SetPageTitle,
    MovePage,
    DeletePages,
    InsertLink,
    SetFolderTitle,
    ReorderFolder,
    MoveFolder,
    DeleteFolder,
    RepairProject,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationWarningCode {
    CleanupPending,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationWarning {
    pub code: OperationWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationFailure {
    pub code: FractalErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationReceipt {
    pub operation: MutationKind,
    pub changes: Vec<ProjectChange>,
    pub warnings: Vec<OperationWarning>,
}

impl MutationReceipt {
    pub fn is_noop(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativePageDraft {
    pub title: String,
    pub content_html: String,
    pub style_css: String,
    pub metadata_html: String,
    pub head_links_html: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryTransactionStatus {
    Pending,
    CommittedCleanupPending,
    Malformed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryTransaction {
    pub path: ProjectPath,
    pub status: RecoveryTransactionStatus,
    pub affected: Vec<ProjectPath>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "repair", rename_all = "snake_case")]
pub enum ProposedRepair {
    MovePath {
        from: ProjectPath,
        to: ProjectPath,
        entry: ProjectEntryKind,
    },
    AppendFolderOrder {
        metadata: ProjectPath,
        additions: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueCode {
    InvalidProject,
    UnsupportedVersion,
    RecoveryRequired,
    RecoveryStateMalformed,
    CleanupPending,
    RepairRequired,
    ValidationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthIssue {
    pub code: HealthIssueCode,
    pub path: Option<ProjectPath>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectInspection {
    pub openable: bool,
    pub healthy: bool,
    pub recovery: Vec<RecoveryTransaction>,
    pub proposed_repairs: Vec<ProposedRepair>,
    pub validation: Option<ValidationReport>,
    pub issues: Vec<HealthIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryReport {
    pub recovered_transactions: Vec<ProjectPath>,
    pub cleaned_transactions: Vec<ProjectPath>,
    pub changes: Vec<ProjectChange>,
    pub warnings: Vec<OperationWarning>,
    pub failures: Vec<OperationFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairReport {
    pub changes: Vec<ProjectChange>,
    pub warnings: Vec<OperationWarning>,
    pub failures: Vec<OperationFailure>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlExportOptions {
    pub include_derived_links: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlExportReport {
    pub output: PathBuf,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderHtmlExportOptions {
    /// Relative page or folder paths. An empty list exports the whole folder.
    pub selections: Vec<PathBuf>,
    pub number_sections: bool,
    pub include_derived_links: bool,
    /// Skip invalid selected pages instead of refusing the export.
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FolderHtmlExportReport {
    pub output: PathBuf,
    pub pages: Vec<String>,
    pub skipped: Vec<SkippedExportPage>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkippedExportPage {
    pub path: String,
    pub reason: String,
}
