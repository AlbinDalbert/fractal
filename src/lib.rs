#[cfg(feature = "cli")]
pub mod cli;
pub mod document;
mod error;
pub mod graph;
pub mod index;
mod io;
pub mod ops;
mod project;
mod types;
pub mod validation;

pub use document::metadata::{
    page_metadata, page_metadata_report, reset_page_metadata, set_page_summary, set_page_tags,
};
pub use document::notes::{add_note, patch_note, remove_note};
pub use error::{FractalError, FractalErrorCode};
pub use graph::{
    graph_backlinks_report, graph_neighbors_report, graph_notes_report, graph_orphans_report,
    graph_outlinks_report, graph_page, graph_page_report, graph_related_report, load_project_graph,
    neighbor_pages, orphan_pages, page_backlinks, page_notes, page_outlinks, related_pages,
};
pub use index::search::{search_project, search_report};
pub use index::{build_index, load_project_index};
pub use ops::{
    create_directory, create_page, delete_directory, delete_page, editor_page_detail, export_page,
    extract_page_text, import_markdown, init_project, init_project_at, list_editor_pages,
    load_project_manifest, new_page, preflight_delete_page, preflight_rename_page, project_summary,
    read_page_source, rename_page, set_page_title, sync_project, update_editor_page,
    update_page_body, write_page_source,
};
pub use types::{
    EditorLinkDetail, EditorNoteDetail, EditorPageDetail, EditorPageListEntry, EditorPageUpdate,
    FileEntry, GraphEdge, GraphNeighborPage, GraphNode, GraphNoteLink, GraphPageLink,
    GraphRelatedPage, LinkEntry, NoteEntry, OperationEvent, OperationReport, OperationSummary,
    PageCreate, PageDeletePreflight, PageEntry, PageGraphEntry, PageMetadata, PageRename,
    PageRenamePreflight, PageSource, PathMove, ProjectGraph, ProjectIndex, ProjectManifest,
    ProjectSummary, SearchMatch, SearchResult, Theme,
};
pub use validation::{preflight_repair_project, repair_project, validate_project};

pub type Result<T> = std::result::Result<T, FractalError>;

#[cfg(test)]
mod tests;
