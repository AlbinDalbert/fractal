use crate::project::constants::{
    INDEX_PAGE, MANIFEST_FILE, MANIFEST_VERSION, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR,
};
use crate::project::index::{build_index, ensure_page_labels_available};
use crate::project::markdown::{html_to_markdown, markdown_to_html};
use crate::project::paths::{
    load_manifest, page_relative_path, resolve_existing_page, resolve_page_destination,
};
use crate::project::render::{default_stylesheet, render_page_document, stylesheet_href};
use crate::project::types::{OperationEvent, OperationReport, PageSource, ProjectManifest, Theme};
use crate::Result;
use std::fs;
use std::path::Path;

pub fn init_project(project_name: &str) -> Result<OperationReport> {
    init_project_at(project_name, project_name)
}

pub fn init_project_at(
    root: impl AsRef<Path>,
    project_name: impl AsRef<str>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let project_name = project_name.as_ref();
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);
    let index_page = pages_dir.join(INDEX_PAGE);
    let manifest_path = root.join(MANIFEST_FILE);

    if root.exists() {
        return Err(format!("path already exists: {}", root.display()).into());
    }

    fs::create_dir_all(&pages_dir)?;
    fs::create_dir_all(&workspace_dir)?;

    let manifest = ProjectManifest {
        project_name: project_name.to_string(),
        version: MANIFEST_VERSION,
        default_page: format!("{PAGES_DIR}/{INDEX_PAGE}"),
        theme: Theme::default(),
    };

    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    fs::write(workspace_dir.join(STYLE_FILE), default_stylesheet())?;
    fs::write(
        &index_page,
        render_page_document(
            project_name,
            "<p>Fractal project scaffold.</p>",
            Theme::default(),
            stylesheet_href(Path::new(INDEX_PAGE)),
        ),
    )?;

    Ok(OperationReport::from_event(OperationEvent::Created {
        path: root.to_path_buf(),
    }))
}

pub fn load_project_manifest(root: impl AsRef<Path>) -> Result<ProjectManifest> {
    load_manifest(root.as_ref())
}

pub fn new_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;

    let destination = resolve_page_destination(root, page.as_ref())?;
    if destination.exists() {
        return Err(format!("page already exists: {}", destination.display()).into());
    }

    let title = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("could not derive page title from destination path")?;

    let pages_dir = root.join(PAGES_DIR);
    let relative_page = destination.strip_prefix(&pages_dir)?;
    let relative_page_string = relative_page.to_string_lossy().replace('\\', "/");
    ensure_page_labels_available(root, &relative_page_string, title)?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &destination,
        render_page_document(
            title,
            "<p>New Fractal page.</p>",
            manifest.theme,
            stylesheet_href(relative_page),
        ),
    )?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::Created { path: destination });
    report.extend(generated);
    Ok(report)
}

pub fn import_markdown(
    root: impl AsRef<Path>,
    source: impl AsRef<Path>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;

    let source = source.as_ref();
    if source.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err(format!("expected a markdown file: {}", source.display()).into());
    }

    let markdown = fs::read_to_string(source)?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("could not derive page name from source file")?;
    let destination = resolve_page_destination(root, Path::new(stem))?;
    if destination.exists() {
        return Err(format!("page already exists: {}", destination.display()).into());
    }

    let (title, body) = markdown_to_html(stem, &markdown);
    let relative_page = page_relative_path(root, &destination)?;
    let relative_page_string = relative_page.to_string_lossy().replace('\\', "/");
    ensure_page_labels_available(root, &relative_page_string, &title)?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &destination,
        render_page_document(
            &title,
            &body,
            manifest.theme,
            stylesheet_href(&relative_page),
        ),
    )?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::Imported {
        source: source.to_path_buf(),
        destination,
    });
    report.extend(generated);
    Ok(report)
}

pub fn export_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    load_manifest(root)?;

    let page = resolve_existing_page(root, page.as_ref())?;

    let output = output.as_ref();
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let html = fs::read_to_string(&page)?;
    fs::write(output, html_to_markdown(&html))?;
    Ok(OperationReport::from_event(OperationEvent::Exported {
        page,
        output: output.to_path_buf(),
    }))
}

pub fn read_page_source(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<PageSource> {
    let root = root.as_ref();
    load_manifest(root)?;

    let page = resolve_existing_page(root, page.as_ref())?;
    let relative_page = page_relative_path(root, &page)?;
    let path = relative_page.to_string_lossy().replace('\\', "/");
    let html = fs::read_to_string(page)?;

    Ok(PageSource { path, html })
}

pub fn write_page_source(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    html: impl AsRef<str>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    load_manifest(root)?;

    let page = resolve_existing_page(root, page.as_ref())?;
    fs::write(&page, html.as_ref())?;

    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::SavedPage { path: page });
    report.extend(generated);
    Ok(report)
}
