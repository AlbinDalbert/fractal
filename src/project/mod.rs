mod constants;
mod document;
mod graph;
mod html;
mod index;
mod links;
mod markdown;
mod metadata;
mod notes;
mod operations;
mod paths;
mod render;
mod search;
mod sync;
mod types;
mod validation;

pub use graph::{
    graph_backlinks_report, graph_notes_report, graph_orphans_report, graph_outlinks_report,
    graph_page, graph_page_report, graph_related_report, load_project_graph, orphan_pages,
    page_backlinks, page_notes, page_outlinks, related_pages,
};
pub use index::{build_index, load_project_index};
pub use metadata::{
    page_metadata, page_metadata_report, reset_page_metadata, set_page_summary, set_page_tags,
};
pub use notes::{add_note, patch_note, remove_note};
pub use operations::{
    export_page, import_markdown, init_project, init_project_at, load_project_manifest, new_page,
    read_page_source, write_page_source,
};
pub use search::{search_project, search_report};
pub use sync::sync_project;
pub use types::{
    FileEntry, GraphEdge, GraphNode, GraphNoteLink, GraphPageLink, GraphRelatedPage, LinkEntry,
    NoteEntry, OperationEvent, OperationReport, PageEntry, PageGraphEntry, PageMetadata,
    PageSource, ProjectGraph, ProjectIndex, ProjectManifest, SearchMatch, SearchResult, Theme,
};
pub use validation::validate_project;

#[cfg(test)]
mod tests;
