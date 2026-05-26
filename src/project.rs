use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

const WORKSPACE_DIR: &str = ".fractal";
const MANIFEST_FILE: &str = "fractal.json";
const INDEX_FILE: &str = "index.json";
const STYLE_FILE: &str = "style.css";
const PAGES_DIR: &str = "pages";
const INDEX_PAGE: &str = "index.html";
const DEFAULT_VERSION: &str = "0.1";
const DEFAULT_SUMMARY: &str = "Short page summary here.";
const DEFAULT_TAGS: &str = "rust, graphs, parsing";

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project_name: String,
    pub version: u32,
    pub default_page: String,
    #[serde(default)]
    pub theme: Theme,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Self::Dark
    }
}

impl Theme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectIndex {
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageEntry {
    pub path: String,
    pub meta: BTreeMap<String, String>,
}

pub fn init_project(project_name: &str) -> Result<()> {
    let root = Path::new(project_name);
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
        version: 1,
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

    println!("created {}", root.display());
    Ok(())
}

pub fn validate_project(root: impl AsRef<Path>, fix: bool) -> Result<()> {
    let root = root.as_ref();
    if fix {
        fix_project(root)?;
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
        let page = pages_dir.join(&page_path);
        validate_page_metadata(&page)?;
    }

    println!(
        "valid Fractal project: {} ({})",
        manifest.project_name,
        manifest_path.display()
    );
    Ok(())
}

fn fix_project(root: &Path) -> Result<()> {
    let manifest = load_manifest(root)?;
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);

    fs::create_dir_all(&workspace_dir)?;
    fs::create_dir_all(&pages_dir)?;

    let stylesheet = workspace_dir.join(STYLE_FILE);
    if !stylesheet.is_file() {
        fs::write(&stylesheet, default_stylesheet())?;
        println!("fixed {}", stylesheet.display());
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
        println!("fixed {}", default_page.display());
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    for page_path in page_paths {
        let page = pages_dir.join(&page_path);
        fix_page(&page, &page_path, manifest.theme)?;
    }

    Ok(())
}

pub fn build_index(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    load_manifest(root)?;

    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);

    if !workspace_dir.is_dir() {
        return Err(format!("missing workspace directory: {}", workspace_dir.display()).into());
    }

    if !pages_dir.is_dir() {
        return Err(format!("missing pages directory: {}", pages_dir.display()).into());
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();

    let index = ProjectIndex {
        pages: page_paths
            .into_iter()
            .map(|path| {
                let meta = extract_fractal_meta_tags(&pages_dir.join(&path))?;
                Ok(PageEntry { path, meta })
            })
            .collect::<Result<Vec<_>>>()?,
    };

    let index_path = workspace_dir.join(INDEX_FILE);
    fs::write(&index_path, serde_json::to_string_pretty(&index)?)?;

    println!("built {}", index_path.display());
    Ok(())
}

pub fn new_page(root: impl AsRef<Path>, page: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let manifest = load_manifest(root)?;

    let destination = resolve_page_destination(root, page.as_ref())?;
    if destination.exists() {
        return Err(format!("page already exists: {}", destination.display()).into());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let title = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("could not derive page title from destination path")?;

    let pages_dir = root.join(PAGES_DIR);
    let relative_page = destination.strip_prefix(&pages_dir)?;
    fs::write(
        &destination,
        render_page_document(
            title,
            "<p>New Fractal page.</p>",
            manifest.theme,
            stylesheet_href(relative_page),
        ),
    )?;
    build_index(root)?;
    println!("created {}", destination.display());
    Ok(())
}

pub fn add_note(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    trigger: &str,
    content: &str,
) -> Result<()> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;

    if find_note_bounds(&html, &note_id).is_some() {
        return Err(format!("note already exists: {note_id}").into());
    }

    let html = insert_note_into_document(&html, &render_note_aside(&note_id, content))?;
    fs::write(&page, html)?;
    build_index(root)?;
    println!("added note {} to {}", note_id, page.display());
    Ok(())
}

pub fn remove_note(root: impl AsRef<Path>, page: impl AsRef<Path>, trigger: &str) -> Result<()> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;
    let html = remove_note_from_document(&html, &note_id)?;
    fs::write(&page, html)?;
    build_index(root)?;
    println!("removed note {} from {}", note_id, page.display());
    Ok(())
}

pub fn patch_note(
    root: impl AsRef<Path>,
    page: impl AsRef<Path>,
    trigger: &str,
    content: &str,
) -> Result<()> {
    let root = root.as_ref();
    let page = resolve_existing_page(root, page.as_ref())?;
    let note_id = note_id_from_trigger(trigger)?;
    let html = fs::read_to_string(&page)?;
    let html = patch_note_in_document(&html, &note_id, content)?;
    fs::write(&page, html)?;
    build_index(root)?;
    println!("patched note {} in {}", note_id, page.display());
    Ok(())
}

pub fn import_markdown(root: impl AsRef<Path>, source: impl AsRef<Path>) -> Result<()> {
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
    let destination = root.join(PAGES_DIR).join(format!("{stem}.html"));

    fs::write(
        &destination,
        render_page_document(
            stem,
            &format!("<pre>{}</pre>", escape_html(&markdown)),
            manifest.theme,
            stylesheet_href(Path::new(&format!("{stem}.html"))),
        ),
    )?;
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

fn resolve_page_destination(root: &Path, page: &Path) -> Result<PathBuf> {
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

fn resolve_existing_page(root: &Path, page: &Path) -> Result<PathBuf> {
    let destination = resolve_page_destination(root, page)?;
    if !destination.is_file() {
        return Err(format!("page does not exist: {}", destination.display()).into());
    }
    Ok(destination)
}

fn collect_page_paths(root: &Path, current: &Path, pages: &mut Vec<String>) -> Result<()> {
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

fn note_id_from_trigger(trigger: &str) -> Result<String> {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for character in trigger
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_was_dash = false;
            continue;
        }

        if (character.is_whitespace() || character == '-' || character == '_')
            && !previous_was_dash
            && !slug.is_empty()
        {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err("trigger must contain at least one letter or number".into());
    }

    Ok(format!("note-{slug}"))
}

fn insert_note_into_document(html: &str, note: &str) -> Result<String> {
    if let Some((_, section_close_start)) = find_notes_section_bounds(html) {
        let mut updated = String::new();
        updated.push_str(&html[..section_close_start]);
        updated.push_str(note);
        updated.push_str(&html[section_close_start..]);
        return Ok(updated);
    }
    Err("missing notes section in page".into())
}

fn remove_note_from_document(html: &str, note_id: &str) -> Result<String> {
    let (start, end) =
        find_note_bounds(html, note_id).ok_or_else(|| format!("note does not exist: {note_id}"))?;
    let mut updated = String::new();
    updated.push_str(&html[..start]);
    updated.push_str(&html[end..]);
    Ok(updated)
}

fn patch_note_in_document(html: &str, note_id: &str, content: &str) -> Result<String> {
    let (start, end) =
        find_note_bounds(html, note_id).ok_or_else(|| format!("note does not exist: {note_id}"))?;
    let mut updated = String::new();
    updated.push_str(&html[..start]);
    updated.push_str(&render_note_aside(note_id, content));
    updated.push_str(&html[end..]);
    Ok(updated)
}

fn find_notes_section_bounds(html: &str) -> Option<(usize, usize)> {
    let section_start = html.find("  <section data-fractal-notes>")?;
    let section_close_start = html[section_start..].find("  </section>")? + section_start;
    Some((section_start, section_close_start))
}

fn find_note_bounds(html: &str, note_id: &str) -> Option<(usize, usize)> {
    let marker = format!("    <aside id=\"{note_id}\" data-fractal-note>");
    let start = html.find(&marker)?;
    let end = html[start..].find("    </aside>\n")? + start + "    </aside>\n".len();
    Some((start, end))
}

fn render_note_aside(note_id: &str, content: &str) -> String {
    format!(
        "    <aside id=\"{note_id}\" data-fractal-note>\n      <p>{}</p>\n    </aside>\n",
        escape_html(content)
    )
}

fn validate_page_metadata(page: &Path) -> Result<()> {
    let html = fs::read_to_string(page)?;
    if find_notes_section_bounds(&html).is_none() {
        return Err(format!("missing notes section in {}", page.display()).into());
    }

    let meta = extract_fractal_meta_tags(page)?;

    for (name, content) in required_meta_tags() {
        if meta.get(name).map(|value| value.as_str()) != Some(content) {
            return Err(
                format!("missing required meta tag in {}: {}", page.display(), name).into(),
            );
        }
    }

    Ok(())
}

fn fix_page(page: &Path, page_path: &str, theme: Theme) -> Result<()> {
    let mut html = fs::read_to_string(page)?;
    let mut changed = false;
    let meta = extract_fractal_meta_tags(page)?;

    for (name, content) in required_meta_tags() {
        if !meta.contains_key(name) {
            html = insert_before_head_close(
                &html,
                &format!(
                    "    <meta name=\"{name}\" content=\"{}\" />\n",
                    escape_html(content)
                ),
            )?;
            changed = true;
        }
    }

    if !html.contains("rel=\"stylesheet\"") || !html.contains(".fractal/style.css") {
        html = insert_before_head_close(
            &html,
            &format!(
                "    <link rel=\"stylesheet\" href=\"{}\">\n",
                escape_html(&stylesheet_href(Path::new(page_path)))
            ),
        )?;
        changed = true;
    }

    let body_theme = format!("data-fractal-theme=\"{}\"", theme.as_str());
    if !html.contains("data-fractal-theme=") {
        if let Some(body_start) = html.find("<body>") {
            html.replace_range(
                body_start..body_start + "<body>".len(),
                &format!("<body {body_theme}>"),
            );
            changed = true;
        }
    }

    if find_notes_section_bounds(&html).is_none() {
        html =
            insert_before_body_close(&html, "    <section data-fractal-notes>\n    </section>\n")?;
        changed = true;
    }

    if changed {
        fs::write(page, html)?;
        println!("fixed {}", page.display());
    }

    Ok(())
}

fn insert_before_head_close(html: &str, insertion: &str) -> Result<String> {
    let Some(index) = html.find("  </head>") else {
        return Err("missing head close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

fn insert_before_body_close(html: &str, insertion: &str) -> Result<String> {
    let Some(index) = html.find("  </body>") else {
        return Err("missing body close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

fn required_meta_tags() -> [(&'static str, &'static str); 3] {
    [
        ("fractal:version", DEFAULT_VERSION),
        ("fractal:summary", DEFAULT_SUMMARY),
        ("fractal:tags", DEFAULT_TAGS),
    ]
}

fn extract_fractal_meta_tags(page: &Path) -> Result<BTreeMap<String, String>> {
    let html = fs::read_to_string(page)?;
    let mut meta = BTreeMap::new();
    let mut remaining = html.as_str();

    while let Some(start) = remaining.find("<meta") {
        remaining = &remaining[start..];
        let Some(end) = remaining.find('>') else {
            break;
        };

        let tag = &remaining[..=end];
        remaining = &remaining[end + 1..];

        let Some(name) = extract_meta_attribute(tag, "name") else {
            continue;
        };
        if !name.starts_with("fractal:") {
            continue;
        }

        let Some(content) = extract_meta_attribute(tag, "content") else {
            continue;
        };

        meta.insert(name, content);
    }

    Ok(meta)
}

fn extract_meta_attribute(tag: &str, attribute: &str) -> Option<String> {
    let pattern = format!("{attribute}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(unescape_html(&rest[..end]))
}

fn render_page_document(title: &str, body: &str, theme: Theme, stylesheet_href: String) -> String {
    let escaped_title = escape_html(title);
    let escaped_summary = escape_html(DEFAULT_SUMMARY);
    let escaped_tags = escape_html(DEFAULT_TAGS);
    let escaped_stylesheet_href = escape_html(&stylesheet_href);
    let theme = theme.as_str();

    format!(
        "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>{escaped_title}</title>\n    <meta name=\"fractal:version\" content=\"{DEFAULT_VERSION}\" />\n    <meta name=\"fractal:summary\" content=\"{escaped_summary}\" />\n    <meta name=\"fractal:tags\" content=\"{escaped_tags}\" />\n    <link rel=\"stylesheet\" href=\"{escaped_stylesheet_href}\">\n  </head>\n  <body data-fractal-theme=\"{theme}\">\n    <main>\n      <h1>{escaped_title}</h1>\n      {body}\n    </main>\n    <section data-fractal-notes>\n    </section>\n  </body>\n</html>\n"
    )
}

fn stylesheet_href(page_path: &Path) -> String {
    let parent_depth = page_path
        .parent()
        .map(|parent| parent.components().count())
        .unwrap_or(0);
    let mut href = String::new();
    for _ in 0..=parent_depth {
        href.push_str("../");
    }
    href.push_str(".fractal/style.css");
    href
}

fn default_stylesheet() -> &'static str {
    r#"body[data-fractal-theme="dark"] {
  --fractal-background: #19110b;
  --fractal-surface: #26180f;
  --fractal-text: #f6cb5c;
  --fractal-muted: #cca061;
  --fractal-border: #402919;
  --fractal-accent: #e4ae51;
}

body[data-fractal-theme="light"] {
  --fractal-background: #f3eadb;
  --fractal-surface: #fbf6ed;
  --fractal-text: #3a2418;
  --fractal-muted: #7f664c;
  --fractal-border: #ddceb8;
  --fractal-accent: #9a6b2f;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  background: var(--fractal-background);
  color: var(--fractal-text);
  font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  line-height: 1.6;
}

main,
section[data-fractal-notes] {
  width: min(760px, calc(100% - 32px));
  margin: 0 auto;
}

main {
  padding: 56px 0 32px;
}

h1 {
  margin: 0 0 24px;
  font-size: 2.25rem;
  line-height: 1.15;
}

a {
  color: var(--fractal-accent);
}

pre,
aside[data-fractal-note] {
  border: 1px solid var(--fractal-border);
  border-radius: 8px;
  background: var(--fractal-surface);
}

pre {
  overflow-x: auto;
  padding: 16px;
}

section[data-fractal-notes] {
  padding: 0 0 56px;
}

aside[data-fractal-note] {
  margin: 16px 0 0;
  padding: 12px 16px;
  color: var(--fractal-muted);
}
"#
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn unescape_html(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::{
        add_note, collect_page_paths, escape_html, extract_fractal_meta_tags, new_page,
        note_id_from_trigger, patch_note, remove_note, render_page_document,
        resolve_page_destination, stylesheet_href, validate_page_metadata, validate_project,
        PageEntry, ProjectIndex, ProjectManifest, Theme,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("fractal-{name}-{unique}"))
    }

    #[test]
    fn escapes_html_sensitive_characters() {
        assert_eq!(escape_html("&<>"), "&amp;&lt;&gt;");
    }

    #[test]
    fn collects_paths_relative_to_pages_root() {
        let root = temp_dir("pages");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(root.join("index.html"), "").expect("write index");
        fs::write(nested.join("note.html"), "").expect("write nested note");

        let mut pages = Vec::new();
        collect_page_paths(&root, &root, &mut pages).expect("collect page paths");
        pages.sort();

        assert_eq!(pages, vec!["index.html", "nested/note.html"]);

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn page_renderer_includes_required_meta_tags() {
        let html = render_page_document(
            "hello",
            "<p>body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        );

        assert!(html.contains("<meta name=\"fractal:version\" content=\"0.1\" />"));
        assert!(
            html.contains("<meta name=\"fractal:summary\" content=\"Short page summary here.\" />")
        );
        assert!(html.contains("<meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" />"));
        assert!(html.contains("<link rel=\"stylesheet\" href=\"../.fractal/style.css\">"));
        assert!(html.contains("<body data-fractal-theme=\"dark\">"));
        assert!(html.contains("<section data-fractal-notes>"));
    }

    #[test]
    fn stylesheet_href_reaches_workspace_from_nested_pages() {
        assert_eq!(
            stylesheet_href(Path::new("index.html")),
            "../.fractal/style.css"
        );
        assert_eq!(
            stylesheet_href(Path::new("folder/subpage.html")),
            "../../.fractal/style.css"
        );
    }

    #[test]
    fn validate_page_metadata_rejects_missing_notes_section() {
        let root = temp_dir("validate-notes");
        fs::create_dir_all(&root).expect("create temp dir");
        let page = root.join("page.html");
        fs::write(
            &page,
            "<html><head><meta name=\"fractal:version\" content=\"0.1\" /><meta name=\"fractal:summary\" content=\"Short page summary here.\" /><meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" /></head><body></body></html>",
        )
        .expect("write page");

        let error = validate_page_metadata(&page).expect_err("missing notes section should fail");
        assert!(error.to_string().contains("missing notes section"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn validate_fix_repairs_missing_project_scaffold_and_page_markers() {
        let root = temp_dir("validate-fix");
        let pages_dir = root.join("pages");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::write(
            root.join("fractal.json"),
            serde_json::to_string_pretty(&ProjectManifest {
                project_name: "test".to_string(),
                version: 1,
                default_page: "pages/index.html".to_string(),
                theme: Theme::Dark,
            })
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            pages_dir.join("index.html"),
            "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>index</title>\n  </head>\n  <body>\n    <main>\n      <h1>index</h1>\n      <p>body</p>\n    </main>\n  </body>\n</html>\n",
        )
        .expect("write page");

        validate_project(&root, true).expect("fix and validate project");

        assert!(root.join(".fractal/style.css").is_file());
        let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
        assert!(html.contains("<meta name=\"fractal:version\" content=\"0.1\" />"));
        assert!(html.contains("<link rel=\"stylesheet\" href=\"../.fractal/style.css\">"));
        assert!(html.contains("<body data-fractal-theme=\"dark\">"));
        assert!(html.contains("<section data-fractal-notes>"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn page_destination_is_relative_to_pages_root() {
        let destination = resolve_page_destination(Path::new("."), Path::new("nested/secondpage"))
            .expect("resolve destination");

        assert_eq!(destination, PathBuf::from("./pages/nested/secondpage.html"));
    }

    #[test]
    fn page_destination_rejects_parent_traversal() {
        let error = resolve_page_destination(Path::new("."), Path::new("../escape"))
            .expect_err("parent traversal should be rejected");

        assert_eq!(error.to_string(), "page path cannot contain `..`");
    }

    #[test]
    fn note_id_normalizes_trigger_text() {
        assert_eq!(
            note_id_from_trigger("Andromeda Galaxy").expect("normalize trigger"),
            "note-andromeda-galaxy"
        );
        assert_eq!(
            note_id_from_trigger("JAVA").expect("normalize trigger"),
            "note-java"
        );
    }

    #[test]
    fn new_page_rebuilds_index() {
        let root = temp_dir("new-page-index");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::create_dir_all(&workspace_dir).expect("create workspace dir");
        fs::write(
            root.join("fractal.json"),
            serde_json::to_string_pretty(&ProjectManifest {
                project_name: "test".to_string(),
                version: 1,
                default_page: "pages/index.html".to_string(),
                theme: Theme::Dark,
            })
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            pages_dir.join("index.html"),
            render_page_document(
                "index",
                "<p>body</p>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write index page");

        new_page(&root, Path::new("secondpage")).expect("create new page");

        let index: ProjectIndex = serde_json::from_str(
            &fs::read_to_string(workspace_dir.join("index.json")).expect("read index"),
        )
        .expect("parse index");

        assert_eq!(
            index.pages,
            vec![
                PageEntry {
                    path: "index.html".to_string(),
                    meta: [
                        (
                            "fractal:summary".to_string(),
                            "Short page summary here.".to_string()
                        ),
                        (
                            "fractal:tags".to_string(),
                            "rust, graphs, parsing".to_string()
                        ),
                        ("fractal:version".to_string(), "0.1".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                },
                PageEntry {
                    path: "secondpage.html".to_string(),
                    meta: [
                        (
                            "fractal:summary".to_string(),
                            "Short page summary here.".to_string()
                        ),
                        (
                            "fractal:tags".to_string(),
                            "rust, graphs, parsing".to_string()
                        ),
                        ("fractal:version".to_string(), "0.1".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                }
            ]
        );

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn extracts_all_fractal_meta_tags() {
        let root = temp_dir("extract-meta");
        fs::create_dir_all(&root).expect("create temp dir");
        let page = root.join("page.html");
        fs::write(
            &page,
            "<meta name=\"fractal:version\" content=\"0.1\" />\n<meta name=\"fractal:summary\" content=\"Short page summary here.\" />\n<meta name=\"fractal:tags\" content=\"rust, graphs, parsing\" />\n<meta name=\"fractal:extra\" content=\"alpha &amp; beta\" />\n<meta name=\"description\" content=\"ignore me\" />",
        )
        .expect("write page");

        let meta = extract_fractal_meta_tags(&page).expect("extract meta");

        assert_eq!(
            meta,
            [
                ("fractal:extra".to_string(), "alpha & beta".to_string()),
                (
                    "fractal:summary".to_string(),
                    "Short page summary here.".to_string()
                ),
                (
                    "fractal:tags".to_string(),
                    "rust, graphs, parsing".to_string()
                ),
                ("fractal:version".to_string(), "0.1".to_string()),
            ]
            .into_iter()
            .collect()
        );

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn add_note_creates_notes_section_in_requested_page() {
        let root = temp_dir("add-note");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::create_dir_all(&workspace_dir).expect("create workspace dir");
        fs::write(
            root.join("fractal.json"),
            serde_json::to_string_pretty(&ProjectManifest {
                project_name: "test".to_string(),
                version: 1,
                default_page: "pages/index.html".to_string(),
                theme: Theme::Dark,
            })
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            pages_dir.join("index.html"),
            render_page_document(
                "index",
                "<p>body</p>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write index page");

        add_note(&root, Path::new("index"), "java", "my note text").expect("add note");

        let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
        assert!(html.contains("<section data-fractal-notes>"));
        assert!(html.contains("<aside id=\"note-java\" data-fractal-note>"));
        assert!(html.contains("<p>my note text</p>"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn patch_note_updates_existing_note() {
        let root = temp_dir("patch-note");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::create_dir_all(&workspace_dir).expect("create workspace dir");
        fs::write(
            root.join("fractal.json"),
            serde_json::to_string_pretty(&ProjectManifest {
                project_name: "test".to_string(),
                version: 1,
                default_page: "pages/index.html".to_string(),
                theme: Theme::Dark,
            })
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            pages_dir.join("index.html"),
            render_page_document(
                "index",
                "<p>body</p>\n  <section data-fractal-notes>\n    <aside id=\"note-java\" data-fractal-note>\n      <p>old text</p>\n    </aside>\n  </section>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write index page");

        patch_note(&root, Path::new("index"), "java", "new text").expect("patch note");

        let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
        assert!(html.contains("<p>new text</p>"));
        assert!(!html.contains("<p>old text</p>"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn remove_note_keeps_empty_note_section() {
        let root = temp_dir("remove-note");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::create_dir_all(&workspace_dir).expect("create workspace dir");
        fs::write(
            root.join("fractal.json"),
            serde_json::to_string_pretty(&ProjectManifest {
                project_name: "test".to_string(),
                version: 1,
                default_page: "pages/index.html".to_string(),
                theme: Theme::Dark,
            })
            .expect("serialize manifest"),
        )
        .expect("write manifest");
        fs::write(
            pages_dir.join("index.html"),
            render_page_document(
                "index",
                "<p>body</p>\n  <section data-fractal-notes>\n    <aside id=\"note-java\" data-fractal-note>\n      <p>old text</p>\n    </aside>\n  </section>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write index page");

        remove_note(&root, Path::new("index"), "java").expect("remove note");

        let html = fs::read_to_string(pages_dir.join("index.html")).expect("read page");
        assert!(!html.contains("note-java"));
        assert!(html.contains("<section data-fractal-notes>"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }
}
