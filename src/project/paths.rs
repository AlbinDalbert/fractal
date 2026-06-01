use crate::project::constants::{MANIFEST_FILE, PAGES_DIR};
use crate::project::types::ProjectManifest;
use crate::Result;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) fn file_kind(path: &str) -> &'static str {
    if is_html_path(path) {
        "page"
    } else {
        "asset"
    }
}

pub(super) fn is_html_path(path: &str) -> bool {
    Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("html")
}

pub(super) fn load_manifest(root: &Path) -> Result<ProjectManifest> {
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(format!("missing manifest: {}", manifest_path.display()).into());
    }

    let manifest = fs::read_to_string(&manifest_path)?;
    Ok(serde_json::from_str(&manifest)?)
}

pub(super) fn normalize_project_path(root: &Path, page: &Path) -> PathBuf {
    if page.is_absolute() {
        page.to_path_buf()
    } else {
        root.join(page)
    }
}

pub(super) fn resolve_page_destination(root: &Path, page: &Path) -> Result<PathBuf> {
    if page.is_absolute() {
        return Err("page path must be relative to pages/".into());
    }

    for component in page.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => {
                return Err("page path cannot contain `..`".into());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("page path must be relative to pages/".into());
            }
        }
    }

    let mut relative = page.to_path_buf();
    if relative.extension().is_none() {
        relative.set_extension("html");
    }

    if relative.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return Err("page path must end in .html or omit the extension".into());
    }

    Ok(root.join(PAGES_DIR).join(relative))
}

pub(super) fn resolve_existing_page(root: &Path, page: &Path) -> Result<PathBuf> {
    let destination = resolve_page_destination(root, page)?;
    if !destination.is_file() {
        return Err(format!("page does not exist: {}", destination.display()).into());
    }
    Ok(destination)
}

pub(super) fn collect_page_paths(
    root: &Path,
    current: &Path,
    pages: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            collect_page_paths(root, &path, pages)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let relative = path.strip_prefix(root)?;
        pages.push(relative.to_string_lossy().replace('\\', "/"));
    }

    Ok(())
}
