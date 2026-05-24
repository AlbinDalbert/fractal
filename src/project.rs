use crate::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE: &str = "fractal.json";
const PAGES_DIR: &str = "pages";
const INDEX_PAGE: &str = "index.html";

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project_name: String,
    pub version: u32,
    pub default_page: String,
}

pub fn init_project(project_name: &str) -> Result<()> {
    let root = Path::new(project_name);
    let pages_dir = root.join(PAGES_DIR);
    let index_page = pages_dir.join(INDEX_PAGE);
    let manifest_path = root.join(MANIFEST_FILE);

    if root.exists() {
        return Err(format!("path already exists: {}", root.display()).into());
    }

    fs::create_dir_all(&pages_dir)?;

    let manifest = ProjectManifest {
        project_name: project_name.to_string(),
        version: 1,
        default_page: format!("{PAGES_DIR}/{INDEX_PAGE}"),
    };

    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    fs::write(&index_page, default_index_page(project_name))?;

    println!("created {}", root.display());
    Ok(())
}

pub fn validate_project(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let manifest_path = root.join(MANIFEST_FILE);
    let pages_dir = root.join(PAGES_DIR);
    let manifest = load_manifest(root)?;

    if !pages_dir.is_dir() {
        return Err(format!("missing pages directory: {}", pages_dir.display()).into());
    }

    let default_page = root.join(&manifest.default_page);
    if !default_page.is_file() {
        return Err(format!("missing default page: {}", default_page.display()).into());
    }

    println!(
        "valid Fractal project: {} ({})",
        manifest.project_name,
        manifest_path.display()
    );
    Ok(())
}

pub fn import_markdown(root: impl AsRef<Path>, source: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    load_manifest(root)?;

    let source = source.as_ref();
    if source.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return Err(format!("expected a markdown file: {}", source.display()).into());
    }

    let markdown = fs::read_to_string(source)?;
    let stem = source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("could not derive page name from source file")?;
    let destination = root.join(PAGES_DIR).join(format!("{stem}.html"));

    fs::write(&destination, imported_page_shell(stem, &markdown))?;
    println!("imported {} -> {}", source.display(), destination.display());
    Ok(())
}

pub fn export_page(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<()> {
    let root = root.as_ref();
    load_manifest(root)?;

    let page = normalize_project_path(root, page.as_ref());
    if page.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return Err(format!("expected an html page: {}", page.display()).into());
    }
    if !page.is_file() {
        return Err(format!("page does not exist: {}", page.display()).into());
    }

    let output = output.as_ref();
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    // This is a scaffolding export step. Real format conversion comes later.
    fs::copy(&page, output)?;
    println!("exported {} -> {}", page.display(), output.display());
    Ok(())
}

fn load_manifest(root: &Path) -> Result<ProjectManifest> {
    let manifest_path = root.join(MANIFEST_FILE);
    if !manifest_path.is_file() {
        return Err(format!("missing manifest: {}", manifest_path.display()).into());
    }

    let manifest = fs::read_to_string(&manifest_path)?;
    Ok(serde_json::from_str(&manifest)?)
}

fn normalize_project_path(root: &Path, page: &Path) -> PathBuf {
    if page.is_absolute() {
        page.to_path_buf()
    } else {
        root.join(page)
    }
}

fn default_index_page(project_name: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>{project_name}</title>\n  </head>\n  <body>\n    <main>\n      <h1>{project_name}</h1>\n      <p>Fractal project scaffold.</p>\n    </main>\n  </body>\n</html>\n"
    )
}

fn imported_page_shell(title: &str, markdown: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>{title}</title>\n  </head>\n  <body>\n    <main data-fractal-source=\"markdown\">\n      <h1>{title}</h1>\n      <pre>{}</pre>\n    </main>\n  </body>\n</html>\n",
        escape_html(markdown)
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::escape_html;

    #[test]
    fn escapes_html_sensitive_characters() {
        assert_eq!(escape_html("&<>"), "&amp;&lt;&gt;");
    }
}
