pub(crate) mod constants;
pub(crate) mod paths;

pub use crate::document::metadata::{
    page_metadata, page_metadata_report, reset_page_metadata, set_page_summary, set_page_tags,
};
pub use crate::document::notes::{add_note, patch_note, remove_note};
pub use crate::graph::{
    graph_backlinks_report, graph_neighbors_report, graph_notes_report, graph_orphans_report,
    graph_outlinks_report, graph_page, graph_page_report, graph_related_report, load_project_graph,
    neighbor_pages, orphan_pages, page_backlinks, page_notes, page_outlinks, related_pages,
};
pub use crate::index::search::{search_project, search_report};
pub use crate::index::{build_index, load_project_index};
pub use crate::ops::{
    create_directory, create_page, delete_directory, delete_page, export_page, extract_page_text,
    import_markdown, init_project, init_project_at, load_project_manifest, new_page,
    preflight_delete_page, preflight_rename_page, project_summary, read_page_source, rename_page,
    sync_project, write_page_source,
};
pub use crate::ops::{
    editor_page_detail, list_editor_pages, set_page_title, update_editor_page, update_page_body,
};
pub use crate::types::{
    EditorLinkDetail, EditorNoteDetail, EditorPageDetail, EditorPageListEntry, EditorPageUpdate,
    FileEntry, GraphEdge, GraphNeighborPage, GraphNode, GraphNoteLink, GraphPageLink,
    GraphRelatedPage, LinkEntry, NoteEntry, OperationEvent, OperationReport, PageCreate,
    PageDeletePreflight, PageEntry, PageGraphEntry, PageMetadata, PageRename, PageRenamePreflight,
    PageSource, ProjectGraph, ProjectIndex, ProjectManifest, ProjectSummary, SearchMatch,
    SearchResult, Theme,
};
pub use crate::validation::{preflight_repair_project, repair_project, validate_project};
