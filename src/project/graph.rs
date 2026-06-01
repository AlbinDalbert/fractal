use crate::project::constants::{GRAPH_FILE, GRAPH_VERSION, PAGES_DIR, WORKSPACE_DIR};
use crate::project::links::{is_external_href, resolve_page_href};
use crate::project::paths::load_manifest;
use crate::project::types::{
    GraphEdge, GraphNode, GraphPageLink, LinkEntry, PageGraphEntry, ProjectGraph, ProjectIndex,
};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

pub fn show_graph_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<()> {
    print!("{}", graph_page_report(root.as_ref(), page.as_ref())?);
    Ok(())
}

pub fn show_graph_orphans(root: impl AsRef<Path>) -> Result<()> {
    print!("{}", graph_orphans_report(root.as_ref())?);
    Ok(())
}

pub(super) fn build_project_graph(index: &ProjectIndex) -> ProjectGraph {
    let page_paths = index
        .pages
        .iter()
        .map(|page| page.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut note_ids = BTreeSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for page in &index.pages {
        nodes.push(GraphNode {
            id: page_node_id(&page.path),
            kind: "page".to_string(),
            label: page.title.clone(),
            path: Some(page.path.clone()),
        });

        for note in &page.notes {
            let note_id = note_node_id(&page.path, &note.id);
            note_ids.insert(note_id.clone());
            nodes.push(GraphNode {
                id: note_id.clone(),
                kind: "note".to_string(),
                label: note.label.clone(),
                path: Some(page.path.clone()),
            });
            edges.push(GraphEdge {
                from: page_node_id(&page.path),
                to: note_id,
                kind: "contains_note".to_string(),
                text: Some(note.label.clone()),
                href: Some(format!("#{}", note.id)),
            });
        }
    }

    for page in &index.pages {
        for link in &page.links {
            if let Some((target, kind)) =
                graph_target_for_link(&page.path, link, &page_paths, &note_ids)
            {
                edges.push(GraphEdge {
                    from: page_node_id(&page.path),
                    to: target,
                    kind: kind.to_string(),
                    text: Some(link.text.clone()),
                    href: Some(link.href.clone()),
                });
            }
        }
    }

    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.text.cmp(&right.text))
            .then_with(|| left.href.cmp(&right.href))
    });
    edges.dedup_by(|left, right| {
        left.from == right.from
            && left.to == right.to
            && left.kind == right.kind
            && left.text == right.text
            && left.href == right.href
    });

    let pages = build_page_graph_entries(index, &edges);

    ProjectGraph {
        version: GRAPH_VERSION,
        nodes,
        edges,
        pages,
    }
}

pub(super) fn graph_page_report(root: &Path, page: &Path) -> Result<String> {
    let graph = load_project_graph(root)?;
    let page_path = normalize_graph_page_path(root, page)?;
    let entry = graph
        .pages
        .iter()
        .find(|entry| entry.path == page_path)
        .ok_or_else(|| format!("page not found in graph: {page_path}"))?;

    let mut report = String::new();
    report.push_str(&format!("{}\n", entry.path));
    push_page_links(&mut report, "outlinks", &entry.outlinks);
    push_page_links(&mut report, "backlinks", &entry.backlinks);
    Ok(report)
}

pub(super) fn graph_orphans_report(root: &Path) -> Result<String> {
    let graph = load_project_graph(root)?;
    let orphans = graph
        .pages
        .iter()
        .filter(|entry| entry.backlinks.is_empty())
        .collect::<Vec<_>>();

    let mut report = String::new();
    report.push_str("orphan pages\n");
    if orphans.is_empty() {
        report.push_str("  (none)\n");
        return Ok(report);
    }

    for entry in orphans {
        report.push_str(&format!(
            "  - {} ({} outlink{})\n",
            entry.path,
            entry.outlinks.len(),
            if entry.outlinks.len() == 1 { "" } else { "s" }
        ));
    }
    Ok(report)
}

fn load_project_graph(root: &Path) -> Result<ProjectGraph> {
    load_manifest(root)?;
    let graph_path = root.join(WORKSPACE_DIR).join(GRAPH_FILE);
    if !graph_path.is_file() {
        return Err(format!(
            "missing graph file: {}. Run `fractal index build` or `fractal sync` first.",
            graph_path.display()
        )
        .into());
    }

    let graph: ProjectGraph = serde_json::from_str(&fs::read_to_string(&graph_path)?)?;
    if graph.version != GRAPH_VERSION {
        return Err(format!(
            "unsupported graph version in {}: {} (expected {})",
            graph_path.display(),
            graph.version,
            GRAPH_VERSION
        )
        .into());
    }

    Ok(graph)
}

fn normalize_graph_page_path(root: &Path, page: &Path) -> Result<String> {
    let page = if page.is_absolute() {
        page.strip_prefix(root.join(PAGES_DIR))
            .map_err(|_| "page path must be inside pages/")?
            .to_path_buf()
    } else {
        let mut components = page.components();
        match components.next() {
            Some(Component::Normal(prefix)) if prefix == PAGES_DIR => {
                components.as_path().to_path_buf()
            }
            _ => page.to_path_buf(),
        }
    };

    for component in page.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => return Err("page path cannot contain `..`".into()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("page path must be relative to pages/".into());
            }
        }
    }

    let mut page = page;
    if page.extension().is_none() {
        page.set_extension("html");
    }
    if page.extension().and_then(|extension| extension.to_str()) != Some("html") {
        return Err("page path must end in .html or omit the extension".into());
    }

    Ok(page.to_string_lossy().replace('\\', "/"))
}

fn push_page_links(report: &mut String, label: &str, links: &[GraphPageLink]) {
    report.push_str(&format!("{label}:\n"));
    if links.is_empty() {
        report.push_str("  (none)\n");
        return;
    }

    for link in links {
        report.push_str(&format!("  - {} ({})\n", link.page, link.text));
    }
}

fn graph_target_for_link(
    page_path: &str,
    link: &LinkEntry,
    page_paths: &BTreeSet<&str>,
    note_ids: &BTreeSet<String>,
) -> Option<(String, &'static str)> {
    if link.scope == "note" && link.href.starts_with('#') {
        let note_id = note_node_id(page_path, link.href.trim_start_matches('#'));
        return note_ids
            .contains(&note_id)
            .then_some((note_id, "links_to_note"));
    }

    if link.scope == "external" || is_external_href(&link.href) {
        return None;
    }

    let target_path = resolve_page_href(page_path, &link.href)?;
    page_paths
        .contains(target_path.as_str())
        .then(|| (page_node_id(&target_path), "links_to_page"))
}

fn build_page_graph_entries(index: &ProjectIndex, edges: &[GraphEdge]) -> Vec<PageGraphEntry> {
    let mut entries = index
        .pages
        .iter()
        .map(|page| {
            (
                page.path.clone(),
                PageGraphEntry {
                    path: page.path.clone(),
                    outlinks: Vec::new(),
                    backlinks: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for edge in edges.iter().filter(|edge| edge.kind == "links_to_page") {
        let Some(source) = edge.from.strip_prefix("page:") else {
            continue;
        };
        let Some(target) = edge.to.strip_prefix("page:") else {
            continue;
        };
        let text = edge.text.clone().unwrap_or_default();

        if let Some(entry) = entries.get_mut(source) {
            entry.outlinks.push(GraphPageLink {
                page: target.to_string(),
                text: text.clone(),
            });
        }

        if let Some(entry) = entries.get_mut(target) {
            entry.backlinks.push(GraphPageLink {
                page: source.to_string(),
                text,
            });
        }
    }

    entries
        .into_values()
        .map(|mut entry| {
            entry.outlinks.sort();
            entry.outlinks.dedup();
            entry.backlinks.sort();
            entry.backlinks.dedup();
            entry
        })
        .collect()
}

fn page_node_id(path: &str) -> String {
    format!("page:{path}")
}

pub(super) fn note_node_id(page_path: &str, note_id: &str) -> String {
    format!("note:{page_path}#{note_id}")
}
