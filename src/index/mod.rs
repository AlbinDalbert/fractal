pub mod search;

use crate::document::PageDocument;
use crate::graph::build_project_graph;
use crate::graph::links::{
    link_label_key, normalize_link_label, page_label_from_path, page_link_labels,
};
use crate::io::fs::atomic_write;
use crate::project::constants::{GRAPH_FILE, INDEX_FILE, INDEX_VERSION, PAGES_DIR, WORKSPACE_DIR};
use crate::project::paths::{collect_page_paths, file_kind, is_html_path, load_manifest};
use crate::types::{
    FileEntry, OperationEvent, OperationReport, PageEntry, ProjectGraph, ProjectIndex,
};
use crate::{FractalError, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn build_index(root: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let index = build_project_index(root)?;
    write_generated_project_data(root, &index)
}

pub fn load_project_index(root: impl AsRef<Path>) -> Result<ProjectIndex> {
    let root = root.as_ref();
    load_manifest(root)?;
    let index_path = root.join(WORKSPACE_DIR).join(INDEX_FILE);
    if !index_path.is_file() {
        return Err(FractalError::not_found(format!(
            "missing index file: {}. Run `fractal index build` or `fractal sync` first.",
            index_path.display()
        )));
    }

    let index: ProjectIndex = serde_json::from_str(&fs::read_to_string(&index_path)?)?;
    if index.version != INDEX_VERSION {
        return Err(FractalError::unsupported_version(format!(
            "unsupported index version in {}: {} (expected {})",
            index_path.display(),
            index.version,
            INDEX_VERSION
        )));
    }

    Ok(index)
}

pub(crate) fn build_project_index(root: &Path) -> Result<ProjectIndex> {
    load_manifest(root)?;

    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);

    if !workspace_dir.is_dir() {
        return Err(FractalError::invalid_project(format!(
            "missing workspace directory: {}",
            workspace_dir.display()
        )));
    }

    if !pages_dir.is_dir() {
        return Err(FractalError::invalid_project(format!(
            "missing pages directory: {}",
            pages_dir.display()
        )));
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
    validate_unique_page_labels(&pages)?;

    Ok(ProjectIndex {
        version: INDEX_VERSION,
        files,
        pages,
    })
}

pub(crate) fn ensure_page_labels_available(root: &Path, path: &str, title: &str) -> Result<()> {
    ensure_page_labels_available_for(root, None, path, title)
}

pub(crate) fn ensure_page_labels_available_for(
    root: &Path,
    current_path: Option<&str>,
    path: &str,
    title: &str,
) -> Result<()> {
    let index = build_project_index(root)?;
    let mut pages = index
        .pages
        .into_iter()
        .filter(|page| Some(page.path.as_str()) != current_path)
        .collect::<Vec<_>>();
    pages.push(PageEntry {
        path: path.to_string(),
        title: normalize_link_label(title),
        meta: BTreeMap::new(),
        notes: Vec::new(),
        links: Vec::new(),
    });

    validate_unique_page_labels(&pages)
}

pub(crate) fn write_generated_project_data(
    root: &Path,
    index: &ProjectIndex,
) -> Result<OperationReport> {
    let mut report = OperationReport::new();
    report.push(write_project_index(root, index)?);
    report.push(write_project_graph(root, &build_project_graph(index))?);
    Ok(report)
}

fn write_project_index(root: &Path, index: &ProjectIndex) -> Result<OperationEvent> {
    let index_path = root.join(WORKSPACE_DIR).join(INDEX_FILE);
    atomic_write(&index_path, serde_json::to_string_pretty(index)?)?;

    Ok(OperationEvent::Built { path: index_path })
}

fn write_project_graph(root: &Path, graph: &ProjectGraph) -> Result<OperationEvent> {
    let graph_path = root.join(WORKSPACE_DIR).join(GRAPH_FILE);
    atomic_write(&graph_path, serde_json::to_string_pretty(graph)?)?;

    Ok(OperationEvent::Built { path: graph_path })
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

fn validate_unique_page_labels(pages: &[PageEntry]) -> Result<()> {
    let mut owners = BTreeMap::<String, (String, String)>::new();

    for page in pages {
        for label in page_link_labels(&page.path, &page.title) {
            let label = normalize_link_label(&label);
            if label.is_empty() {
                continue;
            }

            let key = link_label_key(&label);
            if let Some((existing_label, existing_path)) = owners.get(&key) {
                if existing_path != &page.path {
                    return Err(FractalError::invalid_project(format!(
                        "duplicate page label `{}` for {} and {} (matches existing label `{}`)",
                        label, existing_path, page.path, existing_label
                    )));
                }
                continue;
            }

            owners.insert(key, (label, page.path.clone()));
        }
    }

    Ok(())
}
