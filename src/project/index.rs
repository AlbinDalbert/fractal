use crate::project::constants::{GRAPH_FILE, INDEX_FILE, PAGES_DIR, WORKSPACE_DIR};
use crate::project::document::PageDocument;
use crate::project::graph::build_project_graph;
use crate::project::links::page_label_from_path;
use crate::project::paths::{collect_page_paths, file_kind, is_html_path, load_manifest};
use crate::project::types::{FileEntry, PageEntry, ProjectGraph, ProjectIndex};
use crate::Result;
use std::fs;
use std::path::Path;

pub fn build_index(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let index = build_project_index(root)?;
    write_generated_project_data(root, &index)?;
    Ok(())
}

pub(super) fn build_project_index(root: &Path) -> Result<ProjectIndex> {
    load_manifest(root)?;

    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);

    if !workspace_dir.is_dir() {
        return Err(format!("missing workspace directory: {}", workspace_dir.display()).into());
    }

    if !pages_dir.is_dir() {
        return Err(format!("missing pages directory: {}", pages_dir.display()).into());
    }

    let mut paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut paths)?;
    paths.sort();

    let files = paths
        .iter()
        .map(|path| FileEntry {
            path: path.clone(),
            kind: file_kind(path).to_string(),
        })
        .collect();

    let pages = paths
        .into_iter()
        .filter(|path| is_html_path(path))
        .map(|path| build_page_entry(&pages_dir, path))
        .collect::<Result<Vec<_>>>()?;

    Ok(ProjectIndex { files, pages })
}

pub(super) fn write_generated_project_data(root: &Path, index: &ProjectIndex) -> Result<()> {
    write_project_index(root, index)?;
    write_project_graph(root, &build_project_graph(index))?;
    Ok(())
}

fn write_project_index(root: &Path, index: &ProjectIndex) -> Result<()> {
    let index_path = root.join(WORKSPACE_DIR).join(INDEX_FILE);
    fs::write(&index_path, serde_json::to_string_pretty(index)?)?;

    println!("built {}", index_path.display());
    Ok(())
}

fn write_project_graph(root: &Path, graph: &ProjectGraph) -> Result<()> {
    let graph_path = root.join(WORKSPACE_DIR).join(GRAPH_FILE);
    fs::write(&graph_path, serde_json::to_string_pretty(graph)?)?;

    println!("built {}", graph_path.display());
    Ok(())
}

fn build_page_entry(pages_dir: &Path, path: String) -> Result<PageEntry> {
    let page = pages_dir.join(&path);
    let html = fs::read_to_string(&page)?;
    let document = PageDocument::parse(&html);
    let meta = document.fractal_meta();
    let title = document
        .title()
        .unwrap_or_else(|| page_label_from_path(&path));
    let notes = document.notes();
    let links = document.links();

    Ok(PageEntry {
        path,
        title,
        meta,
        notes,
        links,
    })
}
