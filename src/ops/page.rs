use crate::document::render::{default_stylesheet, render_page_document, stylesheet_href};
use crate::document::PageDocument;
use crate::graph::build_project_graph;
use crate::graph::links::{normalize_link_label, page_label_from_path, relative_href};
use crate::index::ensure_page_labels_available_for;
use crate::index::{build_index, build_project_index, ensure_page_labels_available};
use crate::io::markdown::{html_to_markdown, markdown_to_html};
use crate::project::constants::{
    INDEX_PAGE, MANIFEST_FILE, MANIFEST_VERSION, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR,
};
use crate::project::paths::{
    collect_page_paths, is_html_path, load_manifest, page_relative_path, resolve_existing_page,
    resolve_page_destination,
};
use crate::types::{
    OperationEvent, OperationReport, PageDeletePreflight, PageRename, PageRenamePreflight,
    PageSource, ProjectManifest, Theme,
};
use crate::validation::validate_page_html_for_project;
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

pub fn preflight_rename_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    rename: PageRename,
) -> Result<PageRenamePreflight> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;
    let source = resolve_existing_page(root, page.as_ref())?;
    let source_relative = page_relative_path(root, &source)?;
    let source_page = source_relative.to_string_lossy().replace('\\', "/");
    let destination = match rename.path.as_deref() {
        Some(path) => resolve_page_destination(root, path)?,
        None => source.clone(),
    };
    let destination_relative = page_relative_path(root, &destination)?;
    let destination_page = destination_relative.to_string_lossy().replace('\\', "/");
    let path_changed = source != destination;

    if !path_changed && rename.title.is_none() {
        return Err("rename requires a new page path, a new title, or both".into());
    }

    if path_changed && destination.exists() {
        return Err(format!("page already exists: {}", destination.display()).into());
    }

    let html = fs::read_to_string(&source)?;
    let document = PageDocument::parse(&html);
    let current_title = document
        .title()
        .unwrap_or_else(|| page_label_from_path(&source_page));
    let title = match rename.title.as_deref() {
        Some(title) => normalize_page_title(title)?,
        None => current_title.clone(),
    };

    ensure_page_labels_available_for(root, Some(&source_page), &destination_page, &title)?;

    let index = build_project_index(root)?;
    let graph = build_project_graph(&index);
    let graph_entry = graph
        .pages
        .into_iter()
        .find(|entry| entry.path == source_page)
        .ok_or_else(|| format!("page not found in graph: {source_page}"))?;

    Ok(PageRenamePreflight {
        source_page: source_page.clone(),
        destination_page,
        source_path: source,
        destination_path: destination,
        title: title.clone(),
        path_changed,
        title_changed: current_title != title,
        updates_default_page: path_changed
            && manifest_default_page_path(root, &manifest)? == source_page,
        backlinks: graph_entry.backlinks,
        outlinks: graph_entry.outlinks,
    })
}

pub fn rename_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    rename: PageRename,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut manifest = load_manifest(root)?;
    let preflight = preflight_rename_page(root, page, rename)?;
    let html = fs::read_to_string(&preflight.source_path)?;
    let document = PageDocument::parse(&html);
    let destination_relative = page_relative_path(root, &preflight.destination_path)?;

    let title_changed = document.set_title(&preflight.title)?;
    document.set_stylesheet_href(&stylesheet_href(&destination_relative))?;
    let moved_page_link_updates = if preflight.path_changed {
        document.rewrite_relative_page_hrefs_for_move(
            &preflight.source_page,
            &preflight.destination_page,
        )
    } else {
        0
    };
    let updated_html = document.to_html()?;

    if let Some(parent) = preflight.destination_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if preflight.path_changed {
        fs::rename(&preflight.source_path, &preflight.destination_path)?;
    }
    fs::write(&preflight.destination_path, updated_html)?;

    let mut report = OperationReport::new();
    if preflight.path_changed {
        report.push(OperationEvent::MovedPage {
            from: preflight.source_path.clone(),
            to: preflight.destination_path.clone(),
        });
    }
    if title_changed {
        report.push(OperationEvent::UpdatedPageTitle {
            page: preflight.destination_path.clone(),
            title: preflight.title.clone(),
        });
    }
    if moved_page_link_updates > 0 {
        report.push(OperationEvent::UpdatedPageLinks {
            page: preflight.destination_path.clone(),
            count: moved_page_link_updates,
        });
    }

    if preflight.path_changed {
        report.extend(rewrite_renamed_page_links(
            root,
            &preflight.source_page,
            &preflight.destination_page,
        )?);
    }

    if preflight.updates_default_page {
        manifest.default_page = format!("{PAGES_DIR}/{}", preflight.destination_page);
        let manifest_path = root.join(MANIFEST_FILE);
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        report.push(OperationEvent::UpdatedProjectManifest {
            path: manifest_path,
        });
    }

    report.extend(build_index(root)?);
    Ok(report)
}

pub fn preflight_delete_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
) -> Result<PageDeletePreflight> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;
    let page = resolve_existing_page(root, page.as_ref())?;
    let relative_page = page_relative_path(root, &page)?;
    let page_path = relative_page.to_string_lossy().replace('\\', "/");
    let index = build_project_index(root)?;
    let graph = build_project_graph(&index);
    let graph_entry = graph
        .pages
        .into_iter()
        .find(|entry| entry.path == page_path)
        .ok_or_else(|| format!("page not found in graph: {page_path}"))?;
    let remaining_pages = index
        .pages
        .iter()
        .filter(|entry| entry.path != page_path)
        .collect::<Vec<_>>();
    let deleting_default = manifest_default_page_path(root, &manifest)? == page_path;

    if remaining_pages.is_empty() {
        return Err("cannot delete the only page in a project".into());
    }

    Ok(PageDeletePreflight {
        page: page_path,
        path: page,
        deleting_default,
        replacement_default_page: deleting_default.then(|| remaining_pages[0].path.clone()),
        backlinks: graph_entry.backlinks,
        outlinks: graph_entry.outlinks,
    })
}

pub fn delete_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut manifest = load_manifest(root)?;
    let preflight = preflight_delete_page(root, page)?;
    fs::remove_file(&preflight.path)?;

    let mut report = OperationReport::new();
    report.push(OperationEvent::PageLinksAffected {
        page: preflight.page.clone(),
        backlinks: preflight.backlinks,
        outlinks: preflight.outlinks,
    });
    report.push(OperationEvent::DeletedPage {
        path: preflight.path,
    });

    report.extend(unwrap_deleted_page_links(root, &preflight.page)?);

    if let Some(default_page) = preflight.replacement_default_page {
        manifest.default_page = format!("{PAGES_DIR}/{default_page}");
        let manifest_path = root.join(MANIFEST_FILE);
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        report.push(OperationEvent::UpdatedProjectManifest {
            path: manifest_path,
        });
    }

    report.extend(build_index(root)?);
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
    let relative_page = page_relative_path(root, &page)?;
    let page_path = relative_page.to_string_lossy().replace('\\', "/");
    validate_page_html_for_project(root, &page_path, html.as_ref())?;
    fs::write(&page, html.as_ref())?;

    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::SavedPage { path: page });
    report.extend(generated);
    Ok(report)
}

pub fn extract_page_text(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<String> {
    let root = root.as_ref();
    load_manifest(root)?;
    let page_path = resolve_existing_page(root, page.as_ref())?;
    PageDocument::from_path(&page_path)?.main_text()
}

fn normalize_page_title(title: &str) -> Result<String> {
    let title = normalize_link_label(title);
    if title.is_empty() {
        return Err("page title cannot be empty".into());
    }
    Ok(title)
}

fn manifest_default_page_path(root: &Path, manifest: &ProjectManifest) -> Result<String> {
    Ok(page_relative_path(root, Path::new(&manifest.default_page))?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn unwrap_deleted_page_links(root: &Path, deleted_path: &str) -> Result<OperationReport> {
    let pages_dir = root.join(PAGES_DIR);
    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    let mut report = OperationReport::new();
    for page_path in page_paths.into_iter().filter(|path| is_html_path(path)) {
        let page = pages_dir.join(&page_path);
        let html = fs::read_to_string(&page)?;
        let document = PageDocument::parse(&html);
        let updated = document.unwrap_generated_page_hrefs(&page_path, deleted_path);
        if updated == 0 {
            continue;
        }

        fs::write(&page, document.to_html()?)?;
        report.push(OperationEvent::UpdatedPageLinks {
            page,
            count: updated,
        });
    }

    Ok(report)
}

fn rewrite_renamed_page_links(
    root: &Path,
    source_path: &str,
    destination_path: &str,
) -> Result<OperationReport> {
    let pages_dir = root.join(PAGES_DIR);
    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    let mut report = OperationReport::new();
    for page_path in page_paths.into_iter().filter(|path| is_html_path(path)) {
        let page = pages_dir.join(&page_path);
        let html = fs::read_to_string(&page)?;
        let document = PageDocument::parse(&html);
        let href = relative_href(&page_path, destination_path);
        let updated = document.rewrite_page_hrefs(&page_path, source_path, &href);
        if updated == 0 {
            continue;
        }

        fs::write(&page, document.to_html()?)?;
        report.push(OperationEvent::UpdatedPageLinks {
            page,
            count: updated,
        });
    }

    Ok(report)
}
