use crate::project::constants::{INDEX_PAGE, MANIFEST_FILE, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR};
use crate::project::document::PageDocument;
use crate::project::index::build_project_index;
use crate::project::notes::is_valid_note_id;
use crate::project::paths::{collect_page_paths, is_html_path, load_manifest};
use crate::project::render::{
    default_stylesheet, render_page_document, required_meta_tags, stylesheet_href,
};
use crate::project::types::{OperationEvent, OperationReport, Theme};
use crate::Result;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn validate_project(root: impl AsRef<Path>, fix: bool) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut report = OperationReport::new();
    if fix {
        report.extend(fix_project(root)?);
    }

    let manifest_path = root.join(MANIFEST_FILE);
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);
    let manifest = load_manifest(root)?;

    if !workspace_dir.is_dir() {
        return Err(format!("missing workspace directory: {}", workspace_dir.display()).into());
    }

    let stylesheet = workspace_dir.join(STYLE_FILE);
    if !stylesheet.is_file() {
        return Err(format!("missing stylesheet: {}", stylesheet.display()).into());
    }

    if !pages_dir.is_dir() {
        return Err(format!("missing pages directory: {}", pages_dir.display()).into());
    }

    let default_page = root.join(&manifest.default_page);
    if !default_page.is_file() {
        return Err(format!("missing default page: {}", default_page.display()).into());
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    for page_path in page_paths {
        if !is_html_path(&page_path) {
            continue;
        }

        let page = pages_dir.join(&page_path);
        validate_page_metadata(&page)?;
    }

    build_project_index(root)?;

    report.push(OperationEvent::ValidProject {
        project_name: manifest.project_name,
        manifest_path,
    });
    Ok(report)
}

fn fix_project(root: &Path) -> Result<OperationReport> {
    let manifest = load_manifest(root)?;
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);
    let mut report = OperationReport::new();

    fs::create_dir_all(&workspace_dir)?;
    fs::create_dir_all(&pages_dir)?;

    let stylesheet = workspace_dir.join(STYLE_FILE);
    if !stylesheet.is_file() {
        fs::write(&stylesheet, default_stylesheet())?;
        report.push(OperationEvent::Fixed { path: stylesheet });
    }

    let default_page = root.join(&manifest.default_page);
    if !default_page.is_file() {
        if let Some(parent) = default_page.parent() {
            fs::create_dir_all(parent)?;
        }

        let title = default_page
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&manifest.project_name);
        let page_path = default_page
            .strip_prefix(&pages_dir)
            .unwrap_or_else(|_| Path::new(INDEX_PAGE));
        fs::write(
            &default_page,
            render_page_document(
                title,
                "<p>Fractal project scaffold.</p>",
                manifest.theme,
                stylesheet_href(page_path),
            ),
        )?;
        report.push(OperationEvent::Fixed {
            path: default_page.clone(),
        });
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    for page_path in page_paths {
        if !is_html_path(&page_path) {
            continue;
        }

        let page = pages_dir.join(&page_path);
        if fix_page(&page, &page_path, manifest.theme)? {
            report.push(OperationEvent::Fixed { path: page });
        }
    }

    Ok(report)
}

pub(super) fn validate_page_metadata(page: &Path) -> Result<()> {
    let document = PageDocument::from_path(page)?;

    match document.notes_section_count() {
        0 => return Err(format!("missing notes section in {}", page.display()).into()),
        1 => {}
        count => {
            return Err(format!(
                "duplicate notes section in {}: {count} sections",
                page.display()
            )
            .into())
        }
    }

    let meta = document.fractal_meta();

    for (name, expected_content) in required_meta_tags() {
        let Some(content) = meta.get(name) else {
            return Err(
                format!("missing required meta tag in {}: {}", page.display(), name).into(),
            );
        };

        if name == "fractal:version" && content != expected_content {
            return Err(format!(
                "unsupported page format version in {}: {} (expected {})",
                page.display(),
                content,
                expected_content
            )
            .into());
        }
    }

    validate_note_ids(page, &document)?;

    Ok(())
}

fn fix_page(page: &Path, page_path: &str, theme: Theme) -> Result<bool> {
    let document = PageDocument::from_path(page)?;
    let mut changed = false;

    for (name, content) in required_meta_tags() {
        if document.ensure_meta_tag(name, content)? {
            changed = true;
        }
    }

    if document.ensure_stylesheet_link(&stylesheet_href(Path::new(page_path)))? {
        changed = true;
    }

    if document.ensure_body_theme(theme.as_str())? {
        changed = true;
    }

    if document.ensure_single_notes_section()? {
        changed = true;
    }

    if changed {
        fs::write(page, document.to_html()?)?;
    }

    Ok(changed)
}

fn validate_note_ids(page: &Path, document: &PageDocument) -> Result<()> {
    let mut seen = BTreeSet::new();

    for element in document
        .document
        .select("section[data-fractal-notes] aside[data-fractal-note]")
        .expect("static selector should parse")
    {
        let attributes = element.attributes.borrow();
        let Some(id) = attributes.get("id") else {
            return Err(format!("missing note id in {}", page.display()).into());
        };

        if !is_valid_note_id(id) {
            return Err(format!("malformed note id in {}: {id}", page.display()).into());
        }

        if !seen.insert(id.to_string()) {
            return Err(format!("duplicate note id in {}: {id}", page.display()).into());
        }
    }

    Ok(())
}
