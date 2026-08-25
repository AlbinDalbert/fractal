use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Native,
    Raw,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mutation {
    pub changed: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
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
