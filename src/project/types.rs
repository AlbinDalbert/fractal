use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project_name: String,
    pub version: u32,
    pub default_page: String,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Self::Dark
    }
}

impl Theme {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub files: Vec<FileEntry>,
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageEntry {
    pub path: String,
    pub title: String,
    pub meta: BTreeMap<String, String>,
    pub notes: Vec<NoteEntry>,
    pub links: Vec<LinkEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct NoteEntry {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkEntry {
    pub href: String,
    pub text: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectGraph {
    pub version: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub pages: Vec<PageGraphEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub text: Option<String>,
    pub href: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageGraphEntry {
    pub path: String,
    pub outlinks: Vec<GraphPageLink>,
    pub backlinks: Vec<GraphPageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphPageLink {
    pub page: String,
    pub text: String,
}
