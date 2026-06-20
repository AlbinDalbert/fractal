use crate::document::render::{default_stylesheet, render_page_document, stylesheet_href};
use crate::document::PageDocument;
use crate::graph::build_project_graph;
use crate::graph::links::{normalize_link_label, page_label_from_path, relative_href};
use crate::index::ensure_page_labels_available_for;
use crate::index::{build_index, build_project_index, ensure_page_labels_available};
use crate::io::fs::atomic_write;
use crate::io::markdown::{html_to_markdown, markdown_to_html};
use crate::project::constants::{
    MANIFEST_FILE, MANIFEST_VERSION, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR,
};
use crate::project::paths::{
    collect_page_paths, is_html_path, load_manifest, page_destination_from_title,
    page_destination_in_directory, page_relative_path, resolve_directory_destination,
    resolve_existing_page, resolve_page_destination,
};
use crate::types::{
    OperationEvent, OperationReport, PageCreate, PageDeletePreflight, PageRename,
    PageRenamePreflight, PageSource, ProjectManifest, Theme,
};
use crate::validation::validate_page_html_for_project;
use crate::{FractalError, Result};
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
    let manifest_path = root.join(MANIFEST_FILE);

    if root.exists() {
        return Err(FractalError::already_exists(format!(
            "path already exists: {}",
            root.display()
        )));
    }

    fs::create_dir_all(&pages_dir)?;
    fs::create_dir_all(&workspace_dir)?;

    let manifest = ProjectManifest {
        project_name: project_name.to_string(),
        version: MANIFEST_VERSION,
        default_page: String::new(),
        theme: Theme::default(),
    };

    atomic_write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    atomic_write(workspace_dir.join(STYLE_FILE), default_stylesheet())?;

    Ok(OperationReport::from_event(OperationEvent::ProjectCreated {
        path: root.to_path_buf(),
    })
    .relative_to(root))
}

pub fn load_project_manifest(root: impl AsRef<Path>) -> Result<ProjectManifest> {
    load_manifest(root.as_ref())
}

pub fn create_directory(
    root: impl AsRef<Path>,
    parent: impl AsRef<Path>,
    name: impl AsRef<str>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    load_manifest(root)?;

    let parent = resolve_directory_destination(root, parent.as_ref())?;
    if !parent.is_dir() {
        return Err(FractalError::not_found(format!(
            "directory does not exist: {}",
            parent.display()
        )));
    }

    let name = name.as_ref().trim();
    if Path::new(name).components().count() != 1 {
        return Err(FractalError::invalid_input(
            "directory name must be a single slug component",
        ));
    }
    let destination_name = resolve_directory_destination(root, Path::new(name))?;
    let destination = parent.join(destination_name.file_name().expect("directory name"));
    if destination.exists() {
        return Err(FractalError::already_exists(format!(
            "directory already exists: {}",
            destination.display()
        )));
    }

    fs::create_dir(&destination)?;
    Ok(
        OperationReport::from_event(OperationEvent::DirectoryCreated { path: destination })
            .relative_to(root),
    )
}

pub fn create_page(root: impl AsRef<Path>, page: PageCreate) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut manifest = load_manifest(root)?;
    let title = normalize_page_title(&page.title)?;
    let directory = page.directory.as_deref().unwrap_or_else(|| Path::new(""));
    let directory_path = resolve_directory_destination(root, directory)?;
    if !directory_path.is_dir() {
        return Err(FractalError::not_found(format!(
            "directory does not exist: {}",
            directory_path.display()
        )));
    }

    let destination = page_destination_in_directory(root, directory, &title)?;
    if destination.exists() {
        return Err(FractalError::already_exists(format!(
            "page already exists: {}",
            destination.display()
        )));
    }

    let pages_dir = root.join(PAGES_DIR);
    let relative_page = destination.strip_prefix(&pages_dir)?;
    let relative_page_string = relative_page.to_string_lossy().replace('\\', "/");
    ensure_page_labels_available(root, &relative_page_string, &title)?;

    atomic_write(
        &destination,
        render_page_document(&title, "", manifest.theme, stylesheet_href(relative_page)),
    )?;
    let updated_manifest = if manifest.default_page.trim().is_empty() {
        manifest.default_page = format!("{PAGES_DIR}/{relative_page_string}");
        let manifest_path = root.join(MANIFEST_FILE);
        atomic_write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        Some(manifest_path)
    } else {
        None
    };

    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::PageCreated { path: destination });
    if let Some(path) = updated_manifest {
        report.push(OperationEvent::ManifestUpdated { path });
    }
    report.extend(generated);
    Ok(report.relative_to(root))
}

pub fn new_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<OperationReport> {
    let page = page.as_ref();
    let directory = page
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let title = page
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| page.to_string_lossy().into_owned());
    create_page(
        root,
        PageCreate {
            directory: directory.map(Path::to_path_buf),
            title,
        },
    )
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
    let html = fs::read_to_string(&source)?;
    let document = PageDocument::parse(&html);
    let current_title = document
        .title()
        .unwrap_or_else(|| page_label_from_path(&source_page));
    let title = match rename.title.as_deref() {
        Some(title) => normalize_page_title(title)?,
        None => current_title.clone(),
    };
    let destination = match rename.path.as_deref() {
        Some(path) => resolve_page_destination(root, path)?,
        None if rename.title.is_some() => page_destination_from_title(root, &title)?,
        None => source.clone(),
    };
    let destination_relative = page_relative_path(root, &destination)?;
    let destination_page = destination_relative.to_string_lossy().replace('\\', "/");
    let path_changed = source != destination;

    if !path_changed && rename.title.is_none() {
        return Err(FractalError::invalid_input(
            "rename requires a new page path, a new title, or both",
        ));
    }

    ensure_page_labels_available_for(root, Some(&source_page), &destination_page, &title)?;

    if path_changed && destination.exists() {
        let existing_page = destination_page.clone();
        if let Ok(existing_html) = fs::read_to_string(&destination) {
            let existing_title = PageDocument::parse(&existing_html)
                .title()
                .unwrap_or_else(|| page_label_from_path(&existing_page));
            if normalize_link_label(&existing_title).to_lowercase()
                == normalize_link_label(&title).to_lowercase()
            {
                return Err(FractalError::invalid_project(format!(
                    "duplicate page label `{}` for {} and {} (matches existing label `{}`)",
                    title, existing_page, source_page, existing_title
                )));
            }
        }
        return Err(FractalError::already_exists(format!(
            "page already exists: {}",
            destination.display()
        )));
    }

    let index = build_project_index(root)?;
    let graph = build_project_graph(&index);
    let graph_entry = graph
        .pages
        .into_iter()
        .find(|entry| entry.path == source_page)
        .ok_or_else(|| {
            FractalError::not_found(format!("page not found in graph: {source_page}"))
        })?;

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
    atomic_write(&preflight.destination_path, updated_html)?;

    let mut report = OperationReport::new();
    if preflight.path_changed {
        report.push(OperationEvent::PageMoved {
            from: preflight.source_path.clone(),
            to: preflight.destination_path.clone(),
        });
    }
    if title_changed {
        report.push(OperationEvent::PageTitleUpdated {
            page: preflight.destination_path.clone(),
            title: preflight.title.clone(),
        });
    }
    if moved_page_link_updates > 0 {
        report.push(OperationEvent::PageLinksRewritten {
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
    if title_changed {
        report.extend(rewrite_renamed_page_link_text(
            root,
            &preflight.destination_page,
            &preflight.title,
        )?);
    }

    if preflight.updates_default_page {
        manifest.default_page = format!("{PAGES_DIR}/{}", preflight.destination_page);
        let manifest_path = root.join(MANIFEST_FILE);
        atomic_write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        report.push(OperationEvent::ManifestUpdated {
            path: manifest_path,
        });
    }

    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
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
        .ok_or_else(|| FractalError::not_found(format!("page not found in graph: {page_path}")))?;
    let remaining_pages = index
        .pages
        .iter()
        .filter(|entry| entry.path != page_path)
        .collect::<Vec<_>>();
    let deleting_default = manifest_default_page_path(root, &manifest)? == page_path;

    if remaining_pages.is_empty() {
        return Err(FractalError::invalid_input(
            "cannot delete the only page in a project",
        ));
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
    report.push(OperationEvent::PageLinkImpact {
        page: preflight.page.clone(),
        backlinks: preflight.backlinks,
        outlinks: preflight.outlinks,
    });
    report.push(OperationEvent::PageDeleted {
        path: preflight.path,
    });

    report.extend(unwrap_deleted_page_links(root, &preflight.page)?);

    if let Some(default_page) = preflight.replacement_default_page {
        manifest.default_page = format!("{PAGES_DIR}/{default_page}");
        let manifest_path = root.join(MANIFEST_FILE);
        atomic_write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        report.push(OperationEvent::ManifestUpdated {
            path: manifest_path,
        });
    }

    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
}

pub fn delete_directory(
    root: impl AsRef<Path>,
    directory: impl AsRef<Path>,
    recursive: bool,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut manifest = load_manifest(root)?;
    let relative_directory =
        crate::project::paths::normalize_page_directory_path(directory.as_ref())?;
    if relative_directory.as_os_str().is_empty() {
        return Err(FractalError::invalid_input(
            "cannot delete the pages root directory",
        ));
    }

    let pages_dir = root.join(PAGES_DIR);
    let directory_path = pages_dir.join(&relative_directory);
    if !directory_path.is_dir() {
        return Err(FractalError::not_found(format!(
            "directory does not exist: {}",
            directory_path.display()
        )));
    }

    let mut deleted_pages = Vec::new();
    collect_page_paths(&directory_path, &directory_path, &mut deleted_pages)?;
    deleted_pages.sort();
    let directory_prefix = relative_directory.to_string_lossy().replace('\\', "/");
    let deleted_pages = deleted_pages
        .into_iter()
        .filter(|path| is_html_path(path))
        .map(|path| format!("{directory_prefix}/{path}"))
        .collect::<Vec<_>>();

    if recursive {
        fs::remove_dir_all(&directory_path)?;
    } else {
        fs::remove_dir(&directory_path)?;
    }

    let mut report = OperationReport::new();
    report.push(OperationEvent::DirectoryDeleted {
        path: directory_path.clone(),
    });

    for deleted_page in &deleted_pages {
        report.push(OperationEvent::PageDeleted {
            path: pages_dir.join(deleted_page),
        });
        report.extend(unwrap_deleted_page_links(root, deleted_page)?);
    }

    let default_page = manifest_default_page_path(root, &manifest).ok();
    if default_page.as_deref().is_some_and(|path| {
        path == directory_prefix || path.starts_with(&format!("{directory_prefix}/"))
    }) {
        let mut remaining_pages = Vec::new();
        collect_page_paths(&pages_dir, &pages_dir, &mut remaining_pages)?;
        remaining_pages.sort();
        manifest.default_page = remaining_pages
            .into_iter()
            .find(|path| is_html_path(path))
            .map(|path| format!("{PAGES_DIR}/{path}"))
            .unwrap_or_default();
        let manifest_path = root.join(MANIFEST_FILE);
        atomic_write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        report.push(OperationEvent::ManifestUpdated {
            path: manifest_path,
        });
    }

    report.extend(build_index(root)?);
    Ok(report.relative_to(root))
}

pub fn import_markdown(
    root: impl AsRef<Path>,
    source: impl AsRef<Path>,
) -> Result<OperationReport> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;

    let source = source.as_ref();
    if source.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err(FractalError::invalid_input(format!(
            "expected a markdown file: {}",
            source.display()
        )));
    }

    let markdown = fs::read_to_string(source)?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            FractalError::invalid_input("could not derive page name from source file")
        })?;
    let destination = resolve_page_destination(root, Path::new(stem))?;
    if destination.exists() {
        return Err(FractalError::already_exists(format!(
            "page already exists: {}",
            destination.display()
        )));
    }

    let (title, body) = markdown_to_html(stem, &markdown);
    let relative_page = page_relative_path(root, &destination)?;
    let relative_page_string = relative_page.to_string_lossy().replace('\\', "/");
    ensure_page_labels_available(root, &relative_page_string, &title)?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    atomic_write(
        &destination,
        render_page_document(
            &title,
            &body,
            manifest.theme,
            stylesheet_href(&relative_page),
        ),
    )?;
    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::PageImported {
        source: source.to_path_buf(),
        destination,
    });
    report.extend(generated);
    Ok(report.relative_to(root))
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
    atomic_write(output, html_to_markdown(&html))?;
    Ok(OperationReport::from_event(OperationEvent::PageExported {
        page,
        output: output.to_path_buf(),
    })
    .relative_to(root))
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
    atomic_write(&page, html.as_ref())?;

    let generated = build_index(root)?;
    let mut report = OperationReport::from_event(OperationEvent::PageSourceUpdated { page });
    report.extend(generated);
    Ok(report.relative_to(root))
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
        return Err(FractalError::invalid_input("page title cannot be empty"));
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

        atomic_write(&page, document.to_html()?)?;
        report.push(OperationEvent::PageLinksRewritten {
            page,
            count: updated,
        });
    }

    Ok(report.relative_to(root))
}

fn rewrite_renamed_page_link_text(
    root: &Path,
    destination_path: &str,
    title: &str,
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
        let updated = document.rewrite_page_link_text(&page_path, destination_path, title);
        if updated == 0 {
            continue;
        }

        atomic_write(&page, document.to_html()?)?;
        report.push(OperationEvent::PageLinksRewritten {
            page,
            count: updated,
        });
    }

    Ok(report.relative_to(root))
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

        atomic_write(&page, document.to_html()?)?;
        report.push(OperationEvent::PageLinksRewritten {
            page,
            count: updated,
        });
    }

    Ok(report.relative_to(root))
}
