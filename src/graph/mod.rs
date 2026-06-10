pub(crate) mod links;

use crate::graph::links::{is_external_href, resolve_page_href};
use crate::project::constants::{GRAPH_FILE, GRAPH_VERSION, WORKSPACE_DIR};
use crate::project::paths::{load_manifest, page_relative_path};
use crate::types::{
    GraphEdge, GraphNeighborPage, GraphNode, GraphNoteLink, GraphPageLink, GraphRelatedPage,
    LinkEntry, PageGraphEntry, ProjectGraph, ProjectIndex,
};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) fn build_project_graph(index: &ProjectIndex) -> ProjectGraph {
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

pub fn load_project_graph(root: impl AsRef<Path>) -> Result<ProjectGraph> {
    let root = root.as_ref();
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

pub fn graph_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<PageGraphEntry> {
    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    load_project_graph(root)?
        .pages
        .into_iter()
        .find(|entry| entry.path == page_path)
        .ok_or_else(|| format!("page not found in graph: {page_path}").into())
}

pub fn orphan_pages(root: impl AsRef<Path>) -> Result<Vec<PageGraphEntry>> {
    Ok(load_project_graph(root)?
        .pages
        .into_iter()
        .filter(|entry| entry.backlinks.is_empty())
        .collect())
}

pub fn page_backlinks(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
) -> Result<Vec<GraphPageLink>> {
    Ok(graph_page(root, page)?.backlinks)
}

pub fn page_outlinks(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<Vec<GraphPageLink>> {
    Ok(graph_page(root, page)?.outlinks)
}

pub fn related_pages(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
) -> Result<Vec<GraphRelatedPage>> {
    let entry = graph_page(root, page)?;
    let mut related = Vec::new();

    related.extend(entry.outlinks.into_iter().map(|link| GraphRelatedPage {
        page: link.page,
        text: link.text,
        direction: "outlink".to_string(),
    }));
    related.extend(entry.backlinks.into_iter().map(|link| GraphRelatedPage {
        page: link.page,
        text: link.text,
        direction: "backlink".to_string(),
    }));

    related.sort();
    related.dedup();
    Ok(related)
}

pub fn neighbor_pages(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    depth: usize,
) -> Result<Vec<GraphNeighborPage>> {
    if depth == 0 {
        return Ok(Vec::new());
    }

    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    let graph = load_project_graph(root)?;
    let pages = graph
        .pages
        .iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    if !pages.contains_key(&page_path) {
        return Err(format!("page not found in graph: {page_path}").into());
    }

    let mut visited = BTreeSet::from([page_path.clone()]);
    let mut frontier = BTreeSet::from([page_path]);
    let mut neighbors = Vec::new();

    for distance in 1..=depth {
        let mut next_frontier = BTreeSet::new();

        for current in &frontier {
            let Some(entry) = pages.get(current) else {
                continue;
            };
            for adjacent in entry
                .outlinks
                .iter()
                .chain(entry.backlinks.iter())
                .map(|link| link.page.as_str())
            {
                if !visited.insert(adjacent.to_string()) {
                    continue;
                }
                next_frontier.insert(adjacent.to_string());
                neighbors.push(GraphNeighborPage {
                    page: adjacent.to_string(),
                    distance,
                });
            }
        }

        if next_frontier.is_empty() {
            break;
        }
        frontier = next_frontier;
    }

    neighbors.sort();
    Ok(neighbors)
}

pub fn page_notes(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<Vec<GraphNoteLink>> {
    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    let graph = load_project_graph(root)?;
    let page_id = page_node_id(&page_path);
    let labels = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.label.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut notes = graph
        .edges
        .into_iter()
        .filter(|edge| edge.from == page_id && edge.kind == "contains_note")
        .filter_map(|edge| {
            let id = edge.to.split('#').next_back()?.to_string();
            let href = edge.href.unwrap_or_else(|| format!("#{id}"));
            let label = labels
                .get(edge.to.as_str())
                .cloned()
                .or(edge.text)
                .unwrap_or_else(|| id.clone());

            Some(GraphNoteLink { id, label, href })
        })
        .collect::<Vec<_>>();

    notes.sort();
    notes.dedup();
    Ok(notes)
}

pub fn graph_page_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let entry = graph_page(root, page)?;

    let mut report = String::new();
    report.push_str(&format!("{}\n", entry.path));
    push_page_links(&mut report, "outlinks", &entry.outlinks);
    push_page_links(&mut report, "backlinks", &entry.backlinks);
    Ok(report)
}

pub fn graph_backlinks_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let entry = graph_page(root, page)?;
    let mut report = String::new();
    report.push_str(&format!("{}\n", entry.path));
    push_page_links(&mut report, "backlinks", &entry.backlinks);
    Ok(report)
}

pub fn graph_outlinks_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let entry = graph_page(root, page)?;
    let mut report = String::new();
    report.push_str(&format!("{}\n", entry.path));
    push_page_links(&mut report, "outlinks", &entry.outlinks);
    Ok(report)
}

pub fn graph_orphans_report(root: impl AsRef<Path>) -> Result<String> {
    let orphans = orphan_pages(root)?;

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

pub fn graph_related_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    let related = related_pages(root, Path::new(&page_path))?;

    let mut report = String::new();
    report.push_str(&format!("{page_path}\nrelated pages:\n"));
    if related.is_empty() {
        report.push_str("  (none)\n");
        return Ok(report);
    }

    for link in related {
        report.push_str(&format!(
            "  - {} ({}: {})\n",
            link.page, link.direction, link.text
        ));
    }
    Ok(report)
}

pub fn graph_neighbors_report(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    depth: usize,
) -> Result<String> {
    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    let neighbors = neighbor_pages(root, Path::new(&page_path), depth)?;

    let mut report = String::new();
    report.push_str(&format!("{page_path}\nneighbors depth {depth}:\n"));
    if neighbors.is_empty() {
        report.push_str("  (none)\n");
        return Ok(report);
    }

    for neighbor in neighbors {
        report.push_str(&format!(
            "  - {} (distance {})\n",
            neighbor.page, neighbor.distance
        ));
    }
    Ok(report)
}

pub fn graph_notes_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let root = root.as_ref();
    let page_path = normalize_graph_page_path(root, page.as_ref())?;
    let notes = page_notes(root, Path::new(&page_path))?;

    let mut report = String::new();
    report.push_str(&format!("{page_path}\nnotes:\n"));
    if notes.is_empty() {
        report.push_str("  (none)\n");
        return Ok(report);
    }

    for note in notes {
        report.push_str(&format!("  - {} ({})\n", note.id, note.label));
    }
    Ok(report)
}

fn normalize_graph_page_path(root: &Path, page: &Path) -> Result<String> {
    Ok(page_relative_path(root, page)?
        .to_string_lossy()
        .replace('\\', "/"))
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

pub(crate) fn note_node_id(page_path: &str, note_id: &str) -> String {
    format!("note:{page_path}#{note_id}")
}
