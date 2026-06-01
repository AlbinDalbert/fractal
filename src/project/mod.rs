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

pub use graph::{graph_orphans_report, graph_page_report};
pub use index::build_index;
pub use notes::{add_note, patch_note, remove_note};
pub use operations::{export_page, import_markdown, init_project, new_page};
pub use sync::sync_project;
pub use types::{
    FileEntry, GraphEdge, GraphNode, GraphPageLink, LinkEntry, NoteEntry, OperationEvent,
    OperationReport, PageEntry, PageGraphEntry, ProjectGraph, ProjectIndex, ProjectManifest, Theme,
};
pub use validation::validate_project;

#[cfg(test)]
mod tests;
