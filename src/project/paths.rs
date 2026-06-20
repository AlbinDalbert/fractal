use crate::project::constants::{MANIFEST_FILE, MANIFEST_VERSION, PAGES_DIR};
use crate::types::ProjectManifest;
use crate::{FractalError, Result};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) fn file_kind(path: &str) -> &'static str {
    if is_html_path(path) {
        "page"
    } else {
        "asset"
    }
}

pub(crate) fn is_html_path(path: &str) -> bool {
    Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("html")
}

pub(crate) fn load_manifest(root: &Path) -> Result<ProjectManifest> {
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(FractalError::invalid_project(format!(
            "missing manifest: {}",
            manifest_path.display()
        )));
    }

    let manifest: ProjectManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    if manifest.version != MANIFEST_VERSION {
        return Err(FractalError::unsupported_version(format!(
            "unsupported manifest version in {}: {} (expected {})",
            manifest_path.display(),
            manifest.version,
            MANIFEST_VERSION
        )));
    }

    Ok(manifest)
}

pub(crate) fn resolve_page_destination(root: &Path, page: &Path) -> Result<PathBuf> {
    let relative = normalize_page_relative_path(page)?;
    Ok(root.join(PAGES_DIR).join(relative))
}

pub(crate) fn page_destination_from_title(root: &Path, title: &str) -> Result<PathBuf> {
    let slug = page_slug_from_title(title)?;
    resolve_page_destination(root, Path::new(&slug))
}

pub(crate) fn page_slug_from_title(title: &str) -> Result<String> {
    let mut slug = String::new();
    let mut previous_separator = false;

    for character in title.trim().to_lowercase().chars() {
        if character.is_ascii_digit() || (character.is_alphabetic() && character.is_lowercase()) {
            slug.push(character);
            previous_separator = false;
        } else if (character.is_whitespace() || matches!(character, '-' | '_' | '/'))
            && !slug.is_empty()
            && !previous_separator
        {
            slug.push('-');
            previous_separator = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err(FractalError::invalid_input(
            "page title must contain linkable text",
        ));
    }

    Ok(slug)
}

pub(crate) fn resolve_existing_page(root: &Path, page: &Path) -> Result<PathBuf> {
    let destination = resolve_page_reference(root, page)?;
    if !destination.is_file() {
        return Err(FractalError::not_found(format!(
            "page does not exist: {}",
            destination.display()
        )));
    }
    Ok(destination)
}

pub(crate) fn page_relative_path(root: &Path, page: &Path) -> Result<PathBuf> {
    let resolved = resolve_page_reference(root, page)?;
    Ok(resolved.strip_prefix(root.join(PAGES_DIR))?.to_path_buf())
}

fn resolve_page_reference(root: &Path, page: &Path) -> Result<PathBuf> {
    let relative = if page.is_absolute() {
        let stripped = page
            .strip_prefix(root.join(PAGES_DIR))
            .map_err(|_| FractalError::invalid_input("page path must be inside pages/"))?
            .to_path_buf();
        normalize_page_relative_path(&stripped)?
    } else {
        normalize_page_relative_path(page)?
    };

    Ok(root.join(PAGES_DIR).join(relative))
}

fn normalize_page_relative_path(page: &Path) -> Result<PathBuf> {
    if page.is_absolute() {
        return Err(FractalError::invalid_input(
            "page path must be relative to pages/",
        ));
    }

    let mut components = page.components().peekable();
    if matches!(
        components.peek(),
        Some(Component::Normal(prefix)) if prefix.to_str() == Some(PAGES_DIR)
    ) {
        components.next();
    }

    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(part) => {
                validate_slug_component(part)?;
                relative.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(FractalError::invalid_input("page path cannot contain `..`"));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(FractalError::invalid_input(
                    "page path must be relative to pages/",
                ));
            }
        }
    }

    if relative.as_os_str().is_empty() {
        return Err(FractalError::invalid_input("page path cannot be empty"));
    }

    if relative.extension().is_none() {
        relative.set_extension("html");
    }

    if relative.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return Err(FractalError::invalid_input(
            "page path must end in .html or omit the extension",
        ));
    }

    Ok(relative)
}

fn validate_slug_component(component: &OsStr) -> Result<()> {
    let Some(component) = component.to_str() else {
        return Err(FractalError::invalid_input(
            "page path components must be valid UTF-8 slugs",
        ));
    };

    let stem = component.strip_suffix(".html").unwrap_or(component);
    if stem.is_empty()
        || stem.starts_with(['-', '_'])
        || stem.ends_with(['-', '_'])
        || stem.contains("--")
        || stem.contains("__")
        || !stem.chars().all(is_page_slug_character)
    {
        return Err(FractalError::invalid_input(format!(
            "page path component must be a lowercase page slug: {component}"
        )));
    }

    Ok(())
}

fn is_page_slug_character(character: char) -> bool {
    character == '-'
        || character == '_'
        || character.is_ascii_digit()
        || (character.is_alphabetic() && character.is_lowercase())
}

pub(crate) fn collect_page_paths(
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
