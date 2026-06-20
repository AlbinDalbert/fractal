use crate::document::metadata::{
    normalize_tags, summary_from_meta, tags_from_meta, SUMMARY_META, TAGS_META,
};
use crate::document::PageDocument;
use crate::graph::build_project_graph;
use crate::graph::links::{normalize_link_label, page_link_text_matches, resolve_page_href};
use crate::index::{build_index, build_project_index, ensure_page_labels_available_for};
use crate::project::paths::{page_relative_path, resolve_existing_page};
use crate::types::{
    EditorLinkDetail, EditorNoteDetail, EditorPageDetail, EditorPageListEntry, EditorPageUpdate,
    LinkEntry, OperationEvent, OperationReport, PageMetadata, PageSource,
};
use crate::validation::{known_page_titles_for_candidate, validate_page_html_for_project};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub fn list_editor_pages(root: impl AsRef<Path>) -> Result<Vec<EditorPageListEntry>> {
    let root = root.as_ref();
    let index = build_project_index(root)?;
    let graph = build_project_graph(&index);
    let graph_pages = graph
        .pages
        .into_iter()
        .map(|page| (page.path.clone(), page))
        .collect::<BTreeMap<_, _>>();

    index
        .pages
        .into_iter()
        .map(|page| {
            let graph_page = graph_pages
                .get(&page.path)
                .ok_or_else(|| format!("missing graph entry for page: {}", page.path))?;
            Ok(EditorPageListEntry {
                path: page.path,
                title: page.title,
                summary: summary_from_meta(&page.meta),
                tags: tags_from_meta(&page.meta),
                backlink_count: graph_page.backlinks.len(),
                outlink_count: graph_page.outlinks.len(),
            })
        })
        .collect()
}

pub fn editor_page_detail(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
) -> Result<EditorPageDetail> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let relative_page = page_relative_path(root, &page)?;
    let path = relative_page.to_string_lossy().replace('\\', "/");
    let html = fs::read_to_string(&page)?;
    let document = PageDocument::parse(&html);

    let index = build_project_index(root)?;
    let page_titles = index
        .pages
        .iter()
        .map(|page| (page.path.clone(), page.title.clone()))
        .collect::<BTreeMap<_, _>>();
    let graph = build_project_graph(&index);
    let page_entry = index
        .pages
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| format!("page not found in index: {path}"))?;
    let graph_entry = graph
        .pages
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| format!("page not found in graph: {path}"))?;

    let metadata = PageMetadata {
        path: path.clone(),
        title: page_entry.title,
        summary: summary_from_meta(&page_entry.meta),
        tags: tags_from_meta(&page_entry.meta),
        meta: page_entry.meta,
    };

    let notes = editor_note_details(&document);
    let note_ids = notes.iter().map(|note| note.id.clone()).collect();

    Ok(EditorPageDetail {
        source: PageSource {
            path: path.clone(),
            html,
        },
        body_html: document.main_body_html()?,
        metadata,
        notes,
        links: editor_link_details(&path, page_entry.links, &page_titles, &note_ids),
        backlinks: graph_entry.backlinks,
        outlinks: graph_entry.outlinks,
    })
}

pub fn update_editor_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    update: EditorPageUpdate,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let relative_page = page_relative_path(root, &page)?;
    let path = relative_page.to_string_lossy().replace('\\', "/");

    if let Some(title) = update.title.as_deref() {
        let title = normalize_editor_title(title)?;
        ensure_page_labels_available_for(root, Some(&path), &path, &title)?;
    }

    let html = fs::read_to_string(&page)?;
    let document = PageDocument::parse(&html);
    let mut report = OperationReport::new();

    if let Some(title) = update.title {
        let title = normalize_editor_title(&title)?;
        if document.set_title(&title)? {
            report.push(OperationEvent::UpdatedPageTitle {
                page: page.clone(),
                title,
            });
        }
    }

    let body_html_was_supplied = update.body_html.is_some();
    if let Some(body_html) = update.body_html {
        if document.set_main_body_html(&body_html)? {
            report.push(OperationEvent::UpdatedPageBody { page: page.clone() });
        }
    }

    if let Some(summary) = update.summary {
        let summary = summary.trim().to_string();
        if document.set_meta_tag(SUMMARY_META, &summary)? {
            report.push(OperationEvent::UpdatedMetadata {
                page: page.clone(),
                name: SUMMARY_META.to_string(),
                content: summary,
            });
        }
    }

    if let Some(tags) = update.tags {
        let tags = normalize_tags(tags.iter().map(String::as_str)).join(", ");
        if document.set_meta_tag(TAGS_META, &tags)? {
            report.push(OperationEvent::UpdatedMetadata {
                page: page.clone(),
                name: TAGS_META.to_string(),
                content: tags,
            });
        }
    }

    if body_html_was_supplied {
        let known_page_titles = known_page_titles_for_candidate(root, &path, &document)?;
        let repaired_links = document.repair_invalid_links(&path, &known_page_titles);
        if repaired_links > 0 {
            report.push(OperationEvent::UpdatedPageLinks {
                page: page.clone(),
                count: repaired_links,
            });
        }
    }

    if !report.events.is_empty() {
        let html = document.to_html()?;
        validate_page_html_for_project(root, &path, &html)?;
        fs::write(&page, html)?;
    }

    report.extend(build_index(root)?);
    Ok(report)
}

pub fn set_page_title(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    title: &str,
) -> Result<OperationReport> {
    update_editor_page(
        root,
        page,
        EditorPageUpdate {
            title: Some(title.to_string()),
            ..EditorPageUpdate::default()
        },
    )
}

pub fn update_page_body(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    body_html: &str,
) -> Result<OperationReport> {
    update_editor_page(
        root,
        page,
        EditorPageUpdate {
            body_html: Some(body_html.to_string()),
            ..EditorPageUpdate::default()
        },
    )
}

fn editor_note_details(document: &PageDocument) -> Vec<EditorNoteDetail> {
    document
        .notes()
        .into_iter()
        .map(|note| {
            let text = document
                .note_node(&note.id)
                .map(|node| node.text_contents().trim().to_string())
                .unwrap_or_default();

            EditorNoteDetail {
                id: note.id,
                label: note.label,
                text,
            }
        })
        .collect()
}

fn editor_link_details(
    page_path: &str,
    links: Vec<LinkEntry>,
    page_titles: &BTreeMap<String, String>,
    note_ids: &BTreeSet<String>,
) -> Vec<EditorLinkDetail> {
    links
        .into_iter()
        .map(|link| {
            let target_note = link
                .href
                .strip_prefix('#')
                .filter(|note_id| note_ids.contains(*note_id))
                .map(str::to_string);
            let target_page = (!link.href.starts_with('#'))
                .then(|| resolve_page_href(page_path, &link.href))
                .flatten()
                .and_then(|target| {
                    page_titles.get(&target).and_then(|title| {
                        page_link_text_matches(&target, title, &link.text).then_some(target)
                    })
                });

            EditorLinkDetail {
                href: link.href,
                text: link.text,
                scope: link.scope,
                target_page,
                target_note,
            }
        })
        .collect()
}

fn normalize_editor_title(title: &str) -> Result<String> {
    let title = normalize_link_label(title);
    if title.is_empty() {
        return Err("page title cannot be empty".into());
    }
    Ok(title)
}
