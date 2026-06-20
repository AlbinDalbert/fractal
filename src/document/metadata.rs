use crate::document::PageDocument;
use crate::graph::links::normalize_link_label;
use crate::index::build_index;
use crate::io::fs::atomic_write;
use crate::project::constants::{DEFAULT_SUMMARY, DEFAULT_TAGS};
use crate::project::paths::{page_relative_path, resolve_existing_page};
use crate::types::{OperationEvent, OperationReport, PageMetadata};
use crate::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) const SUMMARY_META: &str = "fractal:summary";
pub(crate) const TAGS_META: &str = "fractal:tags";

pub fn page_metadata(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<PageMetadata> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let relative_page = page_relative_path(root, &page)?;
    let path = relative_page.to_string_lossy().replace('\\', "/");
    let document = PageDocument::from_path(&page)?;
    let meta = document.fractal_meta();
    let title = document.title().unwrap_or_else(|| path.clone());
    let summary = summary_from_meta(&meta);
    let tags = tags_from_meta(&meta);

    Ok(PageMetadata {
        path,
        title,
        summary,
        tags,
        meta,
    })
}

pub fn set_page_summary(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    summary: &str,
) -> Result<OperationReport> {
    update_page_meta(root, page, SUMMARY_META, summary.trim())
}

pub fn set_page_tags(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    tags: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<OperationReport> {
    let tags = normalize_tags(tags);
    update_page_meta(root, page, TAGS_META, &tags.join(", "))
}

pub fn reset_page_metadata(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = page.as_ref();
    let mut report = set_page_summary(root, page, DEFAULT_SUMMARY)?;
    report.extend(set_page_tags(root, page, DEFAULT_TAGS.split(','))?);
    Ok(report)
}

pub fn page_metadata_report(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let metadata = page_metadata(root, page)?;
    let mut report = String::new();

    report.push_str(&format!("{}\n", metadata.path));
    report.push_str(&format!("title: {}\n", metadata.title));
    report.push_str(&format!(
        "summary: {}\n",
        metadata.summary.unwrap_or_else(|| "(none)".to_string())
    ));
    report.push_str("tags:");
    if metadata.tags.is_empty() {
        report.push_str(" (none)\n");
    } else {
        report.push_str(&format!(" {}\n", metadata.tags.join(", ")));
    }

    Ok(report)
}

fn update_page_meta(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    name: &str,
    content: &str,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let html = fs::read_to_string(&page)?;
    let document = PageDocument::parse(&html);

    let changed = document.set_meta_tag(name, content)?;
    if changed {
        atomic_write(&page, document.to_html()?)?;
    }

    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::UpdatedMetadata {
        page,
        name: name.to_string(),
        content: content.to_string(),
    });
    report.extend(generated);
    Ok(report)
}

pub(crate) fn summary_from_meta(meta: &BTreeMap<String, String>) -> Option<String> {
    meta.get(SUMMARY_META)
        .filter(|summary| !summary.trim().is_empty())
        .cloned()
}

pub(crate) fn tags_from_meta(meta: &BTreeMap<String, String>) -> Vec<String> {
    meta.get(TAGS_META)
        .map(|tags| parse_tags(tags))
        .unwrap_or_default()
}

pub(crate) fn parse_tags(tags: &str) -> Vec<String> {
    normalize_tags(tags.split(','))
}

pub(crate) fn normalize_tags(tags: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();

    for tag in tags {
        for tag in tag.as_ref().split(',') {
            let tag = normalize_link_label(tag);
            let key = tag.to_lowercase();
            if tag.is_empty() || !seen.insert(key) {
                continue;
            }

            normalized.push(tag);
        }
    }

    normalized
}
