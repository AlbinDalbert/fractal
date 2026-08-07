use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub path: String,
    pub title: Option<String>,
    pub text: String,
    pub links: Vec<Link>,
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
pub struct LinkSuggestion {
    pub text: String,
    pub candidates: Vec<LinkCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkCandidate {
    pub page: String,
    pub title: String,
    pub match_kind: MatchKind,
    pub score: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    ExactTitle,
    ExactStem,
    TokenOverlap,
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
