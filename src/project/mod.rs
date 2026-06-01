mod constants;
mod document;
mod graph;
mod html;
mod index;
mod links;
mod markdown;
mod notes;
mod operations;
mod paths;
mod render;
mod sync;
mod types;
mod validation;

pub use graph::{
    graph_orphans_report, graph_page, graph_page_report, load_project_graph, orphan_pages,
};
pub use index::{build_index, load_project_index};
pub use notes::{add_note, patch_note, remove_note};
pub use operations::{
    export_page, import_markdown, init_project, init_project_at, load_project_manifest, new_page,
    read_page_source, write_page_source,
};
pub use sync::sync_project;
pub use types::{
    FileEntry, GraphEdge, GraphNode, GraphPageLink, LinkEntry, NoteEntry, OperationEvent,
    OperationReport, PageEntry, PageGraphEntry, PageSource, ProjectGraph, ProjectIndex,
    ProjectManifest, Theme,
};
pub use validation::validate_project;

#[cfg(test)]
mod tests;
