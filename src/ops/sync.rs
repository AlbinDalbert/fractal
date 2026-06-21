use crate::document::html::{escape_html, escape_html_attribute, find_case_insensitive};
use crate::document::PageDocument;
use crate::graph::links::{
    is_linkable_label, link_label_key, normalize_link_label, page_link_labels, relative_href,
};
use crate::index::{build_project_index, write_generated_project_data};
use crate::ops::mutation::MutationPlan;
use crate::project::constants::PAGES_DIR;
use crate::types::{OperationEvent, OperationReport, ProjectIndex};
use crate::{FractalError, Result};
use kuchiki::NodeRef;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn sync_project(root: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let initial_index = build_project_index(root)?;
    let pages_dir = root.join(PAGES_DIR);
    let mut planned_rewrites = Vec::new();

    for page in &initial_index.pages {
        let path = pages_dir.join(&page.path);
        let html = fs::read_to_string(&path)?;
        let updated = sync_page_links(&html, &page.path, &initial_index)?;
        if updated.html != html {
            planned_rewrites.push(PlannedSyncRewrite {
                path,
                html: updated.html,
                links_written: updated.links_written,
            });
        }
    }

    let mut plan = MutationPlan::new();
    for rewrite in planned_rewrites {
        plan.write_if_changed(
            rewrite.path.clone(),
            rewrite.html.into_bytes(),
            OperationEvent::PageLinksRewritten {
                page: rewrite.path,
                count: rewrite.links_written,
            },
        );
    }

    let mut report = plan.apply()?;
    let synced = report
        .events
        .iter()
        .filter(|event| matches!(event, OperationEvent::PageLinksRewritten { .. }))
        .count();

    let final_index = build_project_index(root)?;
    report.extend(write_generated_project_data(root, &final_index)?);
    report.push(OperationEvent::SyncCompleted {
        pages_updated: synced,
    });
    Ok(report.relative_to(root))
}

struct PlannedSyncRewrite {
    path: std::path::PathBuf,
    html: String,
    links_written: usize,
}

pub(crate) fn sync_page_links(
    html: &str,
    page_path: &str,
    index: &ProjectIndex,
) -> Result<SyncPageLinks> {
    let document = PageDocument::parse(html);
    let main = document
        .document
        .select_first("main")
        .map_err(|_| FractalError::invalid_project("missing main section in page"))?
        .as_node()
        .clone();
    let note_candidates = note_link_candidates(html);
    let project_candidates = project_link_candidates(index, page_path);

    unwrap_generated_links(&main);
    let links_written = link_candidates_in_node(&main, &note_candidates)
        + link_candidates_in_node(&main, &project_candidates);

    Ok(SyncPageLinks {
        html: document.to_html()?,
        links_written,
    })
}

pub(crate) struct SyncPageLinks {
    pub(crate) html: String,
    pub(crate) links_written: usize,
}

fn note_link_candidates(html: &str) -> Vec<LinkCandidate> {
    let document = PageDocument::parse(html);
    let mut candidates = document
        .notes()
        .into_iter()
        .filter(|note| is_linkable_label(&note.label))
        .map(|note| LinkCandidate {
            match_text: note.label,
            href: format!("#{}", note.id),
            scope: "note",
        })
        .collect::<Vec<_>>();

    sort_link_candidates(&mut candidates);
    candidates
}

fn project_link_candidates(index: &ProjectIndex, current_page: &str) -> Vec<LinkCandidate> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();

    for page in &index.pages {
        if page.path == current_page {
            continue;
        }

        for label in page_link_labels(&page.path, &page.title) {
            let label = normalize_link_label(&label);
            if !is_linkable_label(&label) {
                continue;
            }

            let key = link_label_key(&label);
            if seen.insert(key) {
                candidates.push(LinkCandidate {
                    match_text: label,
                    href: relative_href(current_page, &page.path),
                    scope: "page",
                });
            }
        }
    }

    sort_link_candidates(&mut candidates);
    candidates
}

fn sort_link_candidates(candidates: &mut [LinkCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .match_text
            .len()
            .cmp(&left.match_text.len())
            .then_with(|| left.match_text.cmp(&right.match_text))
    });
}

fn unwrap_generated_links(root: &NodeRef) {
    let links = root
        .select("a[data-fractal-link]")
        .expect("static selector should parse")
        .map(|element| element.as_node().clone())
        .collect::<Vec<_>>();

    for link in links {
        let children = link.children().collect::<Vec<_>>();
        for child in children {
            link.insert_before(child);
        }
        link.detach();
    }
}

fn link_candidates_in_node(root: &NodeRef, candidates: &[LinkCandidate]) -> usize {
    if candidates.is_empty() {
        return 0;
    }

    let text_nodes = root
        .descendants()
        .filter(|node| node.as_text().is_some() && !has_skipped_ancestor(node, root))
        .collect::<Vec<_>>();

    text_nodes
        .into_iter()
        .map(|text_node| link_text_node(&text_node, candidates))
        .sum()
}

fn has_skipped_ancestor(node: &NodeRef, root: &NodeRef) -> bool {
    node.ancestors()
        .take_while(|ancestor| ancestor != root)
        .any(|ancestor| {
            let Some(element) = ancestor.as_element() else {
                return false;
            };
            matches!(
                element.name.local.to_string().as_str(),
                "a" | "code" | "pre" | "script" | "style" | "textarea"
            )
        })
}

fn link_text_node(text_node: &NodeRef, candidates: &[LinkCandidate]) -> usize {
    let Some(text) = text_node.as_text() else {
        return 0;
    };
    let text = text.borrow().clone();
    let ranges = link_ranges(&text, candidates);
    if ranges.is_empty() {
        return 0;
    }

    let links_written = ranges.len();
    let mut offset = 0;
    for range in ranges {
        if range.start > offset {
            text_node.insert_before(NodeRef::new_text(&text[offset..range.start]));
        }

        text_node.insert_before(render_link_node(
            &range.candidate.href,
            range.candidate.scope,
            &text[range.start..range.end],
        ));
        offset = range.end;
    }

    if offset < text.len() {
        text_node.insert_before(NodeRef::new_text(&text[offset..]));
    }
    text_node.detach();
    links_written
}

fn link_ranges(text: &str, candidates: &[LinkCandidate]) -> Vec<LinkRange> {
    let mut ranges = Vec::new();

    for candidate in candidates {
        let mut search_start = 0;
        while let Some(start) = find_case_insensitive(text, &candidate.match_text, search_start) {
            let end = start + candidate.match_text.len();
            if is_phrase_boundary(text, start, end) && !ranges_overlap(&ranges, start, end) {
                ranges.push(LinkRange {
                    start,
                    end,
                    candidate: candidate.clone(),
                });
                search_start = end;
            } else {
                search_start = start + 1;
            }
        }
    }

    ranges.sort_by_key(|range| range.start);
    ranges
}

fn ranges_overlap(ranges: &[LinkRange], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|range| start < range.end && end > range.start)
}

fn is_phrase_boundary(text: &str, start: usize, end: usize) -> bool {
    let previous = text[..start].chars().next_back();
    let next = text[end..].chars().next();

    !previous.map(is_link_word_character).unwrap_or(false)
        && !next.map(is_link_word_character).unwrap_or(false)
}

fn is_link_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn render_link_node(href: &str, scope: &str, text: &str) -> NodeRef {
    let document = PageDocument::parse(&format!(
        "<a href=\"{}\" data-fractal-link=\"{}\">{}</a>",
        escape_html_attribute(href),
        escape_html_attribute(scope),
        escape_html(text)
    ));
    let link = document
        .document
        .select_first("a[data-fractal-link]")
        .expect("generated link markup should parse")
        .as_node()
        .clone();
    link.detach();
    link
}

#[derive(Clone)]
struct LinkCandidate {
    match_text: String,
    href: String,
    scope: &'static str,
}

struct LinkRange {
    start: usize,
    end: usize,
    candidate: LinkCandidate,
}
