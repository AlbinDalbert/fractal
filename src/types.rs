use crate::FractalError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReport {
    pub events: Vec<OperationEvent>,
}

impl OperationReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_event(event: OperationEvent) -> Self {
        Self {
            events: vec![event],
        }
    }

    pub fn push(&mut self, event: OperationEvent) {
        self.events.push(event);
    }

    pub fn extend(&mut self, report: OperationReport) {
        self.events.extend(report.events);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationEvent {
    AddedNote {
        page: PathBuf,
        note_id: String,
    },
    Built {
        path: PathBuf,
    },
    Created {
        path: PathBuf,
    },
    Exported {
        page: PathBuf,
        output: PathBuf,
    },
    DeletedPage {
        path: PathBuf,
    },
    Fixed {
        path: PathBuf,
    },
    Imported {
        source: PathBuf,
        destination: PathBuf,
    },
    MovedPage {
        from: PathBuf,
        to: PathBuf,
    },
    PatchedNote {
        page: PathBuf,
        note_id: String,
    },
    PageLinksAffected {
        page: String,
        backlinks: Vec<GraphPageLink>,
        outlinks: Vec<GraphPageLink>,
    },
    RemovedNote {
        page: PathBuf,
        note_id: String,
    },
    UpdatedPageBody {
        page: PathBuf,
    },
    UpdatedPageTitle {
        page: PathBuf,
        title: String,
    },
    UpdatedMetadata {
        page: PathBuf,
        name: String,
        content: String,
    },
    UpdatedPageLinks {
        page: PathBuf,
        count: usize,
    },
    UpdatedProjectManifest {
        path: PathBuf,
    },
    SavedPage {
        path: PathBuf,
    },
    Synced {
        path: PathBuf,
    },
    SyncComplete {
        pages_updated: usize,
    },
    ValidProject {
        project_name: String,
        manifest_path: PathBuf,
    },
    Warning {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectManifest {
    pub project_name: String,
    pub version: u32,
    pub default_page: String,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIndex {
    pub version: u32,
    pub files: Vec<FileEntry>,
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkEntry {
    pub href: String,
    pub text: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageSource {
    pub path: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageMetadata {
    pub path: String,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub meta: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorPageListEntry {
    pub path: String,
    pub title: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub backlink_count: usize,
    pub outlink_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorPageDetail {
    pub source: PageSource,
    pub body_html: String,
    pub metadata: PageMetadata,
    pub notes: Vec<EditorNoteDetail>,
    pub links: Vec<EditorLinkDetail>,
    pub backlinks: Vec<GraphPageLink>,
    pub outlinks: Vec<GraphPageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorNoteDetail {
    pub id: String,
    pub label: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorLinkDetail {
    pub href: String,
    pub text: String,
    pub scope: String,
    pub target_page: Option<String>,
    pub target_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorPageUpdate {
    pub title: Option<String>,
    pub body_html: Option<String>,
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageRename {
    pub path: Option<PathBuf>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageDeletePreflight {
    pub page: String,
    pub path: PathBuf,
    pub deleting_default: bool,
    pub replacement_default_page: Option<String>,
    pub backlinks: Vec<GraphPageLink>,
    pub outlinks: Vec<GraphPageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageRenamePreflight {
    pub source_page: String,
    pub destination_page: String,
    pub source_path: PathBuf,
    pub destination_path: PathBuf,
    pub title: String,
    pub path_changed: bool,
    pub title_changed: bool,
    pub updates_default_page: bool,
    pub backlinks: Vec<GraphPageLink>,
    pub outlinks: Vec<GraphPageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummary {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub project_name: String,
    pub version: u32,
    pub default_page: String,
    pub theme: Theme,
    pub valid: bool,
    pub validation_error: Option<FractalError>,
    pub file_count: usize,
    pub page_count: usize,
    pub asset_count: usize,
    pub note_count: usize,
    pub link_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub orphan_page_count: usize,
    pub generated_index_exists: bool,
    pub generated_graph_exists: bool,
    pub generated_index_fresh: bool,
    pub generated_graph_fresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectGraph {
    pub version: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub pages: Vec<PageGraphEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub text: Option<String>,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphNoteLink {
    pub id: String,
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphRelatedPage {
    pub page: String,
    pub text: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphNeighborPage {
    pub page: String,
    pub distance: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchResult {
    pub path: String,
    pub title: String,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SearchMatch {
    pub field: String,
    pub text: String,
}
