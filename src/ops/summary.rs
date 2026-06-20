use crate::graph::build_project_graph;
use crate::index::build_project_index;
use crate::project::constants::{GRAPH_FILE, INDEX_FILE, MANIFEST_FILE, WORKSPACE_DIR};
use crate::project::paths::load_manifest;
use crate::types::ProjectSummary;
use crate::validation::validate_project;
use crate::Result;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

pub fn project_summary(root: impl AsRef<Path>) -> Result<ProjectSummary> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;
    let validation_error = validate_project(root).err();
    let index = build_project_index(root).ok();
    let graph = index.as_ref().map(build_project_graph);

    let index_path = root.join(WORKSPACE_DIR).join(INDEX_FILE);
    let graph_path = root.join(WORKSPACE_DIR).join(GRAPH_FILE);
    let generated_index_exists = index_path.is_file();
    let generated_graph_exists = graph_path.is_file();
    let generated_index_fresh = index
        .as_ref()
        .map(|index| generated_file_matches(&index_path, index))
        .unwrap_or(false);
    let generated_graph_fresh = graph
        .as_ref()
        .map(|graph| generated_file_matches(&graph_path, graph))
        .unwrap_or(false);

    let file_count = index.as_ref().map(|index| index.files.len()).unwrap_or(0);
    let page_count = index.as_ref().map(|index| index.pages.len()).unwrap_or(0);
    let asset_count = index
        .as_ref()
        .map(|index| {
            index
                .files
                .iter()
                .filter(|file| file.kind != "page")
                .count()
        })
        .unwrap_or(0);
    let note_count = index
        .as_ref()
        .map(|index| index.pages.iter().map(|page| page.notes.len()).sum())
        .unwrap_or(0);
    let link_count = index
        .as_ref()
        .map(|index| index.pages.iter().map(|page| page.links.len()).sum())
        .unwrap_or(0);
    let graph_node_count = graph.as_ref().map(|graph| graph.nodes.len()).unwrap_or(0);
    let graph_edge_count = graph.as_ref().map(|graph| graph.edges.len()).unwrap_or(0);
    let orphan_page_count = graph
        .as_ref()
        .map(|graph| {
            graph
                .pages
                .iter()
                .filter(|page| page.backlinks.is_empty())
                .count()
        })
        .unwrap_or(0);

    Ok(ProjectSummary {
        root: root.to_path_buf(),
        manifest_path: root.join(MANIFEST_FILE),
        project_name: manifest.project_name,
        version: manifest.version,
        default_page: manifest.default_page,
        theme: manifest.theme,
        valid: validation_error.is_none(),
        validation_error,
        file_count,
        page_count,
        asset_count,
        note_count,
        link_count,
        graph_node_count,
        graph_edge_count,
        orphan_page_count,
        generated_index_exists,
        generated_graph_exists,
        generated_index_fresh,
        generated_graph_fresh,
    })
}

fn generated_file_matches<T>(path: &Path, expected: &T) -> bool
where
    T: DeserializeOwned + PartialEq,
{
    match fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str::<T>(&json).ok())
    {
        Some(actual) => actual == *expected,
        None => false,
    }
}
