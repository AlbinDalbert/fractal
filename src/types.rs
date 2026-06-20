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

    pub fn summary(&self) -> OperationSummary {
        OperationSummary::from_events(&self.events)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationSummary {
    pub source_changed: bool,
    pub generated_changed: bool,
    pub manifest_changed: bool,
    pub validation_performed: bool,
    pub created_paths: Vec<PathBuf>,
    pub changed_paths: Vec<PathBuf>,
    pub deleted_paths: Vec<PathBuf>,
    pub moved_paths: Vec<PathMove>,
    pub repaired_paths: Vec<PathBuf>,
    pub pages_changed: Vec<PathBuf>,
    pub links_rewritten_count: usize,
    pub warnings: Vec<String>,
}

impl OperationSummary {
    fn from_events(events: &[OperationEvent]) -> Self {
        let mut summary = Self::default();

        for event in events {
            match event {
                OperationEvent::ProjectCreated { path }
                | OperationEvent::DirectoryCreated { path }
                | OperationEvent::PageCreated { path } => {
                    summary.source_changed = true;
                    push_unique_path(&mut summary.created_paths, path);
                    push_unique_path(&mut summary.changed_paths, path);
                    if matches!(event, OperationEvent::PageCreated { .. }) {
                        push_unique_path(&mut summary.pages_changed, path);
                    }
                }
                OperationEvent::PageImported { destination, .. } => {
                    summary.source_changed = true;
                    push_unique_path(&mut summary.created_paths, destination);
                    push_unique_path(&mut summary.changed_paths, destination);
                    push_unique_path(&mut summary.pages_changed, destination);
                }
                OperationEvent::PageExported { output, .. } => {
                    push_unique_path(&mut summary.created_paths, output);
                    push_unique_path(&mut summary.changed_paths, output);
                }
                OperationEvent::PageDeleted { path }
                | OperationEvent::DirectoryDeleted { path } => {
                    summary.source_changed = true;
                    push_unique_path(&mut summary.deleted_paths, path);
                    if matches!(event, OperationEvent::PageDeleted { .. }) {
                        push_unique_path(&mut summary.pages_changed, path);
                    }
                }
                OperationEvent::PageMoved { from, to } => {
                    summary.source_changed = true;
                    push_unique_move(&mut summary.moved_paths, from, to);
                    push_unique_path(&mut summary.changed_paths, to);
                    push_unique_path(&mut summary.pages_changed, from);
                    push_unique_path(&mut summary.pages_changed, to);
                }
                OperationEvent::NoteAdded { page, .. }
                | OperationEvent::NoteRemoved { page, .. }
                | OperationEvent::NoteUpdated { page, .. }
                | OperationEvent::PageContentUpdated { page }
                | OperationEvent::PageTitleUpdated { page, .. }
                | OperationEvent::PageMetadataUpdated { page, .. }
                | OperationEvent::PageSourceUpdated { page }
                | OperationEvent::PageLinksRewritten { page, .. } => {
                    summary.source_changed = true;
                    push_unique_path(&mut summary.changed_paths, page);
                    push_unique_path(&mut summary.pages_changed, page);
                    if let OperationEvent::PageLinksRewritten { count, .. } = event {
                        summary.links_rewritten_count += count;
                    }
                }
                OperationEvent::ManifestUpdated { path } => {
                    summary.source_changed = true;
                    summary.manifest_changed = true;
                    push_unique_path(&mut summary.changed_paths, path);
                }
                OperationEvent::ProjectRepaired { path, applied } => {
                    if *applied {
                        summary.source_changed = true;
                        push_unique_path(&mut summary.changed_paths, path);
                    }
                    push_unique_path(&mut summary.repaired_paths, path);
                }
                OperationEvent::GeneratedIndexBuilt { path }
                | OperationEvent::GeneratedGraphBuilt { path } => {
                    summary.generated_changed = true;
                    push_unique_path(&mut summary.changed_paths, path);
                }
                OperationEvent::ProjectValidated { .. } => {
                    summary.validation_performed = true;
                }
                OperationEvent::PageLinkImpact { .. } | OperationEvent::SyncCompleted { .. } => {}
                OperationEvent::Warning { message } => {
                    push_unique_string(&mut summary.warnings, message);
                }
            }
        }

        summary
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: &PathBuf) {
    if !paths.contains(path) {
        paths.push(path.clone());
    }
}

fn push_unique_move(moves: &mut Vec<PathMove>, from: &PathBuf, to: &PathBuf) {
    if !moves
        .iter()
        .any(|entry| entry.from == *from && entry.to == *to)
    {
        moves.push(PathMove {
            from: from.clone(),
            to: to.clone(),
        });
    }
}

fn push_unique_string(strings: &mut Vec<String>, value: &str) {
    if !strings.iter().any(|entry| entry == value) {
        strings.push(value.to_string());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationEvent {
    ProjectCreated {
        path: PathBuf,
    },
    DirectoryCreated {
        path: PathBuf,
    },
    PageCreated {
        path: PathBuf,
    },
    PageImported {
        source: PathBuf,
        destination: PathBuf,
    },
    PageExported {
        page: PathBuf,
        output: PathBuf,
    },
    PageDeleted {
        path: PathBuf,
    },
    DirectoryDeleted {
        path: PathBuf,
    },
    PageMoved {
        from: PathBuf,
        to: PathBuf,
    },
    NoteAdded {
        page: PathBuf,
        note_id: String,
    },
    NoteRemoved {
        page: PathBuf,
        note_id: String,
    },
    NoteUpdated {
        page: PathBuf,
        note_id: String,
    },
    PageContentUpdated {
        page: PathBuf,
    },
    PageTitleUpdated {
        page: PathBuf,
        title: String,
    },
    PageMetadataUpdated {
        page: PathBuf,
        name: String,
        content: String,
    },
    PageLinksRewritten {
        page: PathBuf,
        count: usize,
    },
    PageSourceUpdated {
        page: PathBuf,
    },
    PageLinkImpact {
        page: String,
        backlinks: Vec<GraphPageLink>,
        outlinks: Vec<GraphPageLink>,
    },
    ManifestUpdated {
        path: PathBuf,
    },
    ProjectRepaired {
        path: PathBuf,
        applied: bool,
    },
    GeneratedIndexBuilt {
        path: PathBuf,
    },
    GeneratedGraphBuilt {
        path: PathBuf,
    },
    SyncCompleted {
        pages_updated: usize,
    },
    ProjectValidated {
        project_name: String,
        manifest_path: PathBuf,
    },
    Warning {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageCreate {
    pub directory: Option<PathBuf>,
    pub title: String,
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
