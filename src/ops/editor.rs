use crate::document::metadata::{
    normalize_tags, summary_from_meta, tags_from_meta, SUMMARY_META, TAGS_META,
};
use crate::document::PageDocument;
use crate::graph::build_project_graph;
use crate::graph::links::normalize_link_label;
use crate::index::{build_index, build_project_index, ensure_page_labels_available_for};
use crate::project::paths::{page_relative_path, resolve_existing_page};
use crate::types::{
    EditorNoteDetail, EditorPageDetail, EditorPageListEntry, EditorPageUpdate, OperationEvent,
    OperationReport, PageMetadata, PageSource,
};
use crate::Result;
use std::collections::BTreeMap;
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

    Ok(EditorPageDetail {
        source: PageSource { path, html },
        body_html: document.main_body_html()?,
        metadata,
        notes: editor_note_details(&document),
        links: page_entry.links,
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

    if !report.events.is_empty() {
        fs::write(&page, document.to_html()?)?;
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

fn normalize_editor_title(title: &str) -> Result<String> {
    let title = normalize_link_label(title);
    if title.is_empty() {
        return Err("page title cannot be empty".into());
    }
    Ok(title)
}
