use crate::Result;
use kuchiki::traits::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const WORKSPACE_DIR: &str = ".fractal";
const MANIFEST_FILE: &str = "fractal.json";
const INDEX_FILE: &str = "index.json";
const GRAPH_FILE: &str = "graph.json";
const STYLE_FILE: &str = "style.css";
const PAGES_DIR: &str = "pages";
const INDEX_PAGE: &str = "index.html";
const DEFAULT_VERSION: &str = "0.1";
const DEFAULT_SUMMARY: &str = "Short page summary here.";
const DEFAULT_TAGS: &str = "rust, graphs, parsing";
const GRAPH_VERSION: u32 = 1;

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
    pub files: Vec<FileEntry>,
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageEntry {
    pub path: String,
    pub title: String,
    pub meta: BTreeMap<String, String>,
    pub notes: Vec<NoteEntry>,
    pub links: Vec<LinkEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct NoteEntry {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkEntry {
    pub href: String,
    pub text: String,
    pub scope: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectGraph {
    pub version: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub pages: Vec<PageGraphEntry>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub text: Option<String>,
    pub href: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageGraphEntry {
    pub path: String,
    pub outlinks: Vec<GraphPageLink>,
    pub backlinks: Vec<GraphPageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphPageLink {
    pub page: String,
    pub text: String,
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
        if !is_html_path(&page_path) {
            continue;
        }

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
        if !is_html_path(&page_path) {
            continue;
        }

        let page = pages_dir.join(&page_path);
        fix_page(&page, &page_path, manifest.theme)?;
    }

    Ok(())
}

pub fn build_index(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let index = build_project_index(root)?;
    write_generated_project_data(root, &index)?;
    Ok(())
}

pub fn sync_project(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let initial_index = build_project_index(root)?;
    write_generated_project_data(root, &initial_index)?;

    let pages_dir = root.join(PAGES_DIR);
    let mut synced = 0;
    for page in &initial_index.pages {
        let path = pages_dir.join(&page.path);
        let html = fs::read_to_string(&path)?;
        let updated = sync_page_links(&html, &page.path, &initial_index)?;
        if updated != html {
            fs::write(&path, updated)?;
            synced += 1;
            println!("synced {}", path.display());
        }
    }

    let final_index = build_project_index(root)?;
    write_generated_project_data(root, &final_index)?;
    println!("sync complete: {synced} page(s) updated");
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
    let (title, body) = markdown_to_html(stem, &markdown);

    fs::write(
        &destination,
        render_page_document(
            &title,
            &body,
            manifest.theme,
            stylesheet_href(Path::new(&format!("{stem}.html"))),
        ),
    )?;
    build_index(root)?;
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

    let html = fs::read_to_string(&page)?;
    fs::write(output, html_to_markdown(&html))?;
    println!("exported {} -> {}", page.display(), output.display());
    Ok(())
}

fn build_project_index(root: &Path) -> Result<ProjectIndex> {
    load_manifest(root)?;

    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);

    if !workspace_dir.is_dir() {
        return Err(format!("missing workspace directory: {}", workspace_dir.display()).into());
    }

    if !pages_dir.is_dir() {
        return Err(format!("missing pages directory: {}", pages_dir.display()).into());
    }

    let mut paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut paths)?;
    paths.sort();

    let files = paths
        .iter()
        .map(|path| FileEntry {
            path: path.clone(),
            kind: file_kind(path).to_string(),
        })
        .collect();

    let pages = paths
        .into_iter()
        .filter(|path| is_html_path(path))
        .map(|path| build_page_entry(&pages_dir, path))
        .collect::<Result<Vec<_>>>()?;

    Ok(ProjectIndex { files, pages })
}

fn write_generated_project_data(root: &Path, index: &ProjectIndex) -> Result<()> {
    write_project_index(root, index)?;
    write_project_graph(root, &build_project_graph(index))?;
    Ok(())
}

fn write_project_index(root: &Path, index: &ProjectIndex) -> Result<()> {
    let index_path = root.join(WORKSPACE_DIR).join(INDEX_FILE);
    fs::write(&index_path, serde_json::to_string_pretty(index)?)?;

    println!("built {}", index_path.display());
    Ok(())
}

fn write_project_graph(root: &Path, graph: &ProjectGraph) -> Result<()> {
    let graph_path = root.join(WORKSPACE_DIR).join(GRAPH_FILE);
    fs::write(&graph_path, serde_json::to_string_pretty(graph)?)?;

    println!("built {}", graph_path.display());
    Ok(())
}

fn build_page_entry(pages_dir: &Path, path: String) -> Result<PageEntry> {
    let page = pages_dir.join(&path);
    let html = fs::read_to_string(&page)?;
    let document = PageDocument::parse(&html);
    let meta = document.fractal_meta();
    let title = document
        .title()
        .unwrap_or_else(|| page_label_from_path(&path));
    let notes = document.notes();
    let links = document.links();

    Ok(PageEntry {
        path,
        title,
        meta,
        notes,
        links,
    })
}

struct PageDocument {
    document: kuchiki::NodeRef,
}

impl PageDocument {
    fn parse(html: &str) -> Self {
        Self {
            document: kuchiki::parse_html().one(html),
        }
    }

    fn from_path(page: &Path) -> Result<Self> {
        Ok(Self::parse(&fs::read_to_string(page)?))
    }

    fn has_notes_section(&self) -> bool {
        self.document
            .select_first("section[data-fractal-notes]")
            .is_ok()
    }

    fn title(&self) -> Option<String> {
        self.element_text("title")
            .or_else(|| self.element_text("h1"))
            .map(|title| normalize_link_label(&title))
            .filter(|title| !title.is_empty())
    }

    fn fractal_meta(&self) -> BTreeMap<String, String> {
        let mut meta = BTreeMap::new();

        for element in self
            .document
            .select("meta[name][content]")
            .expect("static selector should parse")
        {
            let attributes = element.attributes.borrow();
            let Some(name) = attributes.get("name") else {
                continue;
            };
            if !name.starts_with("fractal:") {
                continue;
            }
            let Some(content) = attributes.get("content") else {
                continue;
            };

            meta.insert(name.to_string(), content.to_string());
        }

        meta
    }

    fn notes(&self) -> Vec<NoteEntry> {
        let selector = if self.has_notes_section() {
            "section[data-fractal-notes] aside[data-fractal-note]"
        } else {
            "aside[data-fractal-note]"
        };
        let mut notes = Vec::new();

        for element in self
            .document
            .select(selector)
            .expect("static selector should parse")
        {
            let attributes = element.attributes.borrow();
            let Some(id) = attributes.get("id") else {
                continue;
            };

            let label = attributes
                .get("data-fractal-trigger")
                .map(str::to_string)
                .unwrap_or_else(|| note_label_from_id(id));
            let label = normalize_link_label(&label);
            if label.is_empty() {
                continue;
            }

            notes.push(NoteEntry {
                id: id.to_string(),
                label,
            });
        }

        notes
    }

    fn links(&self) -> Vec<LinkEntry> {
        let mut links = Vec::new();

        for element in self
            .document
            .select("a[href]")
            .expect("static selector should parse")
        {
            let attributes = element.attributes.borrow();
            let Some(href) = attributes.get("href") else {
                continue;
            };

            let text = normalize_link_label(&element.text_contents());
            if text.is_empty() {
                continue;
            }

            let scope = attributes
                .get("data-fractal-link")
                .map(str::to_string)
                .unwrap_or_else(|| inferred_link_scope(href).to_string());

            links.push(LinkEntry {
                href: href.to_string(),
                text,
                scope,
            });
        }

        links
    }

    fn element_text(&self, selector: &str) -> Option<String> {
        self.document
            .select_first(selector)
            .ok()
            .map(|element| element.text_contents())
    }
}

fn build_project_graph(index: &ProjectIndex) -> ProjectGraph {
    let page_paths = index
        .pages
        .iter()
        .map(|page| page.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut note_ids = BTreeSet::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for page in &index.pages {
        nodes.push(GraphNode {
            id: page_node_id(&page.path),
            kind: "page".to_string(),
            label: page.title.clone(),
            path: Some(page.path.clone()),
        });

        for note in &page.notes {
            let note_id = note_node_id(&page.path, &note.id);
            note_ids.insert(note_id.clone());
            nodes.push(GraphNode {
                id: note_id.clone(),
                kind: "note".to_string(),
                label: note.label.clone(),
                path: Some(page.path.clone()),
            });
            edges.push(GraphEdge {
                from: page_node_id(&page.path),
                to: note_id,
                kind: "contains_note".to_string(),
                text: Some(note.label.clone()),
                href: Some(format!("#{}", note.id)),
            });
        }
    }

    for page in &index.pages {
        for link in &page.links {
            if let Some((target, kind)) =
                graph_target_for_link(&page.path, link, &page_paths, &note_ids)
            {
                edges.push(GraphEdge {
                    from: page_node_id(&page.path),
                    to: target,
                    kind: kind.to_string(),
                    text: Some(link.text.clone()),
                    href: Some(link.href.clone()),
                });
            }
        }
    }

    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.text.cmp(&right.text))
            .then_with(|| left.href.cmp(&right.href))
    });
    edges.dedup_by(|left, right| {
        left.from == right.from
            && left.to == right.to
            && left.kind == right.kind
            && left.text == right.text
            && left.href == right.href
    });

    let pages = build_page_graph_entries(index, &edges);

    ProjectGraph {
        version: GRAPH_VERSION,
        nodes,
        edges,
        pages,
    }
}

fn graph_target_for_link(
    page_path: &str,
    link: &LinkEntry,
    page_paths: &BTreeSet<&str>,
    note_ids: &BTreeSet<String>,
) -> Option<(String, &'static str)> {
    if link.scope == "note" && link.href.starts_with('#') {
        let note_id = note_node_id(page_path, link.href.trim_start_matches('#'));
        return note_ids
            .contains(&note_id)
            .then_some((note_id, "links_to_note"));
    }

    if link.scope == "external" || is_external_href(&link.href) {
        return None;
    }

    let target_path = resolve_page_href(page_path, &link.href)?;
    page_paths
        .contains(target_path.as_str())
        .then(|| (page_node_id(&target_path), "links_to_page"))
}

fn build_page_graph_entries(index: &ProjectIndex, edges: &[GraphEdge]) -> Vec<PageGraphEntry> {
    let mut entries = index
        .pages
        .iter()
        .map(|page| {
            (
                page.path.clone(),
                PageGraphEntry {
                    path: page.path.clone(),
                    outlinks: Vec::new(),
                    backlinks: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for edge in edges.iter().filter(|edge| edge.kind == "links_to_page") {
        let Some(source) = edge.from.strip_prefix("page:") else {
            continue;
        };
        let Some(target) = edge.to.strip_prefix("page:") else {
            continue;
        };
        let text = edge.text.clone().unwrap_or_default();

        if let Some(entry) = entries.get_mut(source) {
            entry.outlinks.push(GraphPageLink {
                page: target.to_string(),
                text: text.clone(),
            });
        }

        if let Some(entry) = entries.get_mut(target) {
            entry.backlinks.push(GraphPageLink {
                page: source.to_string(),
                text,
            });
        }
    }

    entries
        .into_values()
        .map(|mut entry| {
            entry.outlinks.sort();
            entry.outlinks.dedup();
            entry.backlinks.sort();
            entry.backlinks.dedup();
            entry
        })
        .collect()
}

fn page_node_id(path: &str) -> String {
    format!("page:{path}")
}

fn note_node_id(page_path: &str, note_id: &str) -> String {
    format!("note:{page_path}#{note_id}")
}

fn file_kind(path: &str) -> &'static str {
    if is_html_path(path) {
        "page"
    } else {
        "asset"
    }
}

fn is_html_path(path: &str) -> bool {
    Path::new(path).extension().and_then(|ext| ext.to_str()) == Some("html")
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

fn sync_page_links(html: &str, page_path: &str, index: &ProjectIndex) -> Result<String> {
    let (main_start, main_end) =
        find_element_inner_bounds(html, "main").ok_or("missing main section in page")?;

    let main = unwrap_generated_links(&html[main_start..main_end]);
    let note_candidates = note_link_candidates(html);
    let project_candidates = project_link_candidates(index, page_path);
    let main = link_candidates_in_html(&main, &note_candidates);
    let main = link_candidates_in_html(&main, &project_candidates);

    let mut updated = String::new();
    updated.push_str(&html[..main_start]);
    updated.push_str(&main);
    updated.push_str(&html[main_end..]);
    Ok(updated)
}

fn note_link_candidates(html: &str) -> Vec<LinkCandidate> {
    let document = PageDocument::parse(html);
    let mut candidates = document
        .notes()
        .into_iter()
        .filter(|note| is_linkable_label(&note.label))
        .map(|note| LinkCandidate {
            match_text: escape_html(&note.label),
            href: format!("#{}", note.id),
            scope: "note",
        })
        .collect::<Vec<_>>();

    sort_link_candidates(&mut candidates);
    candidates
}

fn project_link_candidates(index: &ProjectIndex, current_page: &str) -> Vec<LinkCandidate> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();

    for page in &index.pages {
        if page.path == current_page {
            continue;
        }

        for label in [page.title.clone(), page_label_from_path(&page.path)] {
            let label = normalize_link_label(&label);
            if !is_linkable_label(&label) {
                continue;
            }

            let key = label.to_ascii_lowercase();
            if seen.insert(key) {
                candidates.push(LinkCandidate {
                    match_text: escape_html(&label),
                    href: relative_href(current_page, &page.path),
                    scope: "page",
                });
            }
        }
    }

    sort_link_candidates(&mut candidates);
    candidates
}

fn sort_link_candidates(candidates: &mut [LinkCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .match_text
            .len()
            .cmp(&left.match_text.len())
            .then_with(|| left.match_text.cmp(&right.match_text))
    });
}

fn inferred_link_scope(href: &str) -> &'static str {
    if href.starts_with('#') {
        "note"
    } else if is_external_href(href) {
        "external"
    } else {
        "page"
    }
}

fn unwrap_generated_links(html: &str) -> String {
    let mut output = String::new();
    let mut offset = 0;

    while let Some(start) = html[offset..].find("<a") {
        let start = offset + start;
        output.push_str(&html[offset..start]);

        let Some(tag_end) = html[start..].find('>') else {
            output.push_str(&html[start..]);
            return output;
        };
        let tag_end = start + tag_end + 1;
        let tag = &html[start..tag_end];

        if html_tag_name(tag).as_deref() == Some("a") && tag.contains("data-fractal-link=\"") {
            if let Some(close_start) = html[tag_end..].find("</a>") {
                let close_start = tag_end + close_start;
                output.push_str(&html[tag_end..close_start]);
                offset = close_start + "</a>".len();
                continue;
            }
        }

        output.push_str(tag);
        offset = tag_end;
    }

    output.push_str(&html[offset..]);
    output
}

fn link_candidates_in_html(html: &str, candidates: &[LinkCandidate]) -> String {
    if candidates.is_empty() {
        return html.to_string();
    }

    let mut output = String::new();
    let mut offset = 0;
    let mut skip_depth = 0usize;

    while let Some(tag_start) = html[offset..].find('<') {
        let tag_start = offset + tag_start;
        let text = &html[offset..tag_start];
        if skip_depth == 0 {
            output.push_str(&link_text_segment(text, candidates));
        } else {
            output.push_str(text);
        }

        let Some(tag_end) = html[tag_start..].find('>') else {
            output.push_str(&html[tag_start..]);
            return output;
        };
        let tag_end = tag_start + tag_end + 1;
        let tag = &html[tag_start..tag_end];
        output.push_str(tag);
        update_link_skip_depth(tag, &mut skip_depth);
        offset = tag_end;
    }

    let text = &html[offset..];
    if skip_depth == 0 {
        output.push_str(&link_text_segment(text, candidates));
    } else {
        output.push_str(text);
    }

    output
}

fn link_text_segment(text: &str, candidates: &[LinkCandidate]) -> String {
    let mut ranges = Vec::new();

    for candidate in candidates {
        let mut search_start = 0;
        while let Some(start) = find_case_insensitive(text, &candidate.match_text, search_start) {
            let end = start + candidate.match_text.len();
            if is_phrase_boundary(text, start, end)
                && !ranges_overlap(&ranges, start, end)
                && !is_inside_html_entity(text, start, end)
            {
                ranges.push(LinkRange {
                    start,
                    end,
                    candidate: candidate.clone(),
                });
                search_start = end;
            } else {
                search_start = start + 1;
            }
        }
    }

    if ranges.is_empty() {
        return text.to_string();
    }

    ranges.sort_by_key(|range| range.start);

    let mut output = String::new();
    let mut offset = 0;
    for range in ranges {
        output.push_str(&text[offset..range.start]);
        output.push_str(&format!(
            "<a href=\"{}\" data-fractal-link=\"{}\">{}</a>",
            escape_html(&range.candidate.href),
            escape_html(range.candidate.scope),
            &text[range.start..range.end]
        ));
        offset = range.end;
    }
    output.push_str(&text[offset..]);
    output
}

fn find_case_insensitive(haystack: &str, needle: &str, start: usize) -> Option<usize> {
    if needle.is_empty() || start > haystack.len() || needle.len() > haystack.len() {
        return None;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    for index in start..=haystack.len() - needle.len() {
        let end = index + needle.len();
        if !haystack.is_char_boundary(index) || !haystack.is_char_boundary(end) {
            continue;
        }

        if haystack_bytes[index..end].eq_ignore_ascii_case(needle_bytes) {
            return Some(index);
        }
    }

    None
}

fn ranges_overlap(ranges: &[LinkRange], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|range| start < range.end && end > range.start)
}

fn is_phrase_boundary(text: &str, start: usize, end: usize) -> bool {
    let previous = text[..start].chars().next_back();
    let next = text[end..].chars().next();

    !previous.map(is_link_word_character).unwrap_or(false)
        && !next.map(is_link_word_character).unwrap_or(false)
}

fn is_link_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_inside_html_entity(text: &str, start: usize, end: usize) -> bool {
    let Some(entity_start) = text[..start].rfind('&') else {
        return false;
    };
    if text[..start]
        .rfind(';')
        .map(|semicolon| semicolon > entity_start)
        .unwrap_or(false)
    {
        return false;
    }

    text[end..]
        .find(';')
        .map(|semicolon| {
            let entity = &text[entity_start..end + semicolon + 1];
            !entity.contains(char::is_whitespace)
        })
        .unwrap_or(false)
}

fn update_link_skip_depth(tag: &str, skip_depth: &mut usize) {
    let Some(name) = html_tag_name(tag) else {
        return;
    };
    if !matches!(
        name.as_str(),
        "a" | "code" | "pre" | "script" | "style" | "textarea"
    ) {
        return;
    }

    if is_closing_tag(tag) {
        *skip_depth = skip_depth.saturating_sub(1);
    } else if !is_self_closing_tag(tag) {
        *skip_depth += 1;
    }
}

fn find_element_inner_bounds(html: &str, element: &str) -> Option<(usize, usize)> {
    let open_pattern = format!("<{element}");
    let open_start = html.find(&open_pattern)?;
    let open_end = html[open_start..].find('>')? + open_start + 1;
    let close_pattern = format!("</{element}>");
    let close_start = html[open_end..].find(&close_pattern)? + open_end;

    Some((open_end, close_start))
}

fn html_tag_name(tag: &str) -> Option<String> {
    let inner = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .trim_end_matches('/')
        .trim();
    if inner.starts_with('!') || inner.starts_with('?') {
        return None;
    }

    let inner = inner.strip_prefix('/').unwrap_or(inner).trim_start();
    let name = inner
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn is_closing_tag(tag: &str) -> bool {
    tag.trim_start_matches('<').trim_start().starts_with('/')
}

fn is_self_closing_tag(tag: &str) -> bool {
    tag.trim_end().ends_with("/>")
}

fn note_label_from_id(note_id: &str) -> String {
    note_id
        .strip_prefix("note-")
        .unwrap_or(note_id)
        .replace('-', " ")
}

fn page_label_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace('-', " ").replace('_', " "))
        .map(|stem| normalize_link_label(&stem))
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn normalize_link_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_linkable_label(label: &str) -> bool {
    label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .count()
        >= 2
}

fn relative_href(from_page: &str, target_page: &str) -> String {
    let mut from_parts = from_page.split('/').collect::<Vec<_>>();
    if !from_parts.is_empty() {
        from_parts.pop();
    }

    let target_parts = target_page.split('/').collect::<Vec<_>>();
    let mut common = 0;
    while common < from_parts.len()
        && common < target_parts.len()
        && from_parts[common] == target_parts[common]
    {
        common += 1;
    }

    let mut href_parts = Vec::new();
    for _ in common..from_parts.len() {
        href_parts.push("..");
    }
    href_parts.extend(target_parts[common..].iter().copied());

    if href_parts.is_empty() {
        target_page.to_string()
    } else {
        href_parts.join("/")
    }
}

fn resolve_page_href(from_page: &str, href: &str) -> Option<String> {
    if href.starts_with('#') {
        return Some(from_page.to_string());
    }
    if is_external_href(href) || href.starts_with('/') {
        return None;
    }

    let href_without_fragment = href.split('#').next().unwrap_or(href);
    let href_path = href_without_fragment
        .split('?')
        .next()
        .unwrap_or(href_without_fragment);
    if href_path.is_empty() {
        return Some(from_page.to_string());
    }

    let base = Path::new(from_page).parent().unwrap_or_else(|| Path::new(""));
    normalize_project_relative_path(&base.join(href_path))
}

fn normalize_project_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!parts.is_empty()).then(|| parts.join("/"))
}

fn is_external_href(href: &str) -> bool {
    href.contains("://") || href.starts_with("mailto:") || href.starts_with("tel:")
}

#[derive(Clone)]
struct LinkCandidate {
    match_text: String,
    href: String,
    scope: &'static str,
}

struct LinkRange {
    start: usize,
    end: usize,
    candidate: LinkCandidate,
}

fn validate_page_metadata(page: &Path) -> Result<()> {
    let document = PageDocument::from_path(page)?;
    if !document.has_notes_section() {
        return Err(format!("missing notes section in {}", page.display()).into());
    }

    let meta = document.fractal_meta();

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
    let meta = PageDocument::parse(&html).fractal_meta();

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
        if let Some(updated) = insert_body_attribute(&html, &body_theme) {
            html = updated;
            changed = true;
        }
    }

    if !PageDocument::parse(&html).has_notes_section() {
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
    let Some(index) = find_case_insensitive(html, "</head>", 0) else {
        return Err("missing head close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

fn insert_before_body_close(html: &str, insertion: &str) -> Result<String> {
    let Some(index) = find_case_insensitive(html, "</body>", 0) else {
        return Err("missing body close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

fn insert_body_attribute(html: &str, attribute: &str) -> Option<String> {
    let body_start = find_case_insensitive(html, "<body", 0)?;
    let body_end = html[body_start..].find('>')? + body_start;

    let mut updated = String::new();
    updated.push_str(&html[..body_end]);
    updated.push(' ');
    updated.push_str(attribute);
    updated.push_str(&html[body_end..]);
    Some(updated)
}

fn required_meta_tags() -> [(&'static str, &'static str); 3] {
    [
        ("fractal:version", DEFAULT_VERSION),
        ("fractal:summary", DEFAULT_SUMMARY),
        ("fractal:tags", DEFAULT_TAGS),
    ]
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

fn markdown_to_html(default_title: &str, markdown: &str) -> (String, String) {
    let blocks = parse_markdown_blocks(markdown);
    let title = match blocks.first() {
        Some(MarkdownBlock::Heading { level: 1, text }) => text.clone(),
        _ => default_title.to_string(),
    };
    let mut body = String::new();

    for (index, block) in blocks.iter().enumerate() {
        if index == 0 && matches!(block, MarkdownBlock::Heading { level: 1, .. }) {
            continue;
        }

        if !body.is_empty() {
            body.push('\n');
            body.push_str("      ");
        }

        match block {
            MarkdownBlock::Heading { level, text } => {
                body.push_str(&format!("<h{level}>{}</h{level}>", escape_html(text)));
            }
            MarkdownBlock::Paragraph(text) => {
                body.push_str(&format!("<p>{}</p>", escape_html(text)));
            }
        }
    }

    (title, body)
}

fn parse_markdown_blocks(markdown: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            continue;
        }

        if let Some((level, text)) = parse_markdown_heading(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MarkdownBlock::Heading {
                level,
                text: text.to_string(),
            });
        } else {
            paragraph.push(trimmed);
        }
    }

    flush_paragraph(&mut blocks, &mut paragraph);
    blocks
}

fn parse_markdown_heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }

    let rest = &line[level..];
    rest.strip_prefix(' ')
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| (level, text))
}

fn flush_paragraph(blocks: &mut Vec<MarkdownBlock>, paragraph: &mut Vec<&str>) {
    if paragraph.is_empty() {
        return;
    }

    blocks.push(MarkdownBlock::Paragraph(paragraph.join(" ")));
    paragraph.clear();
}

fn html_to_markdown(html: &str) -> String {
    let Some(main_start) = html.find("    <main>") else {
        return String::new();
    };
    let content_start = main_start + "    <main>".len();
    let main_end = html[content_start..]
        .find("    </main>")
        .map(|index| content_start + index)
        .unwrap_or(html.len());

    let mut markdown = String::new();
    for line in html[content_start..main_end].lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((level, text)) = parse_html_heading(trimmed) {
            push_markdown_block(&mut markdown, &format!("{} {}", "#".repeat(level), text));
        } else if let Some(text) = parse_html_paragraph(trimmed) {
            push_markdown_block(&mut markdown, &text);
        }
    }

    markdown
}

fn parse_html_heading(line: &str) -> Option<(usize, String)> {
    for level in 1..=6 {
        let opening = format!("<h{level}>");
        let closing = format!("</h{level}>");
        if let Some(text) = line
            .strip_prefix(&opening)
            .and_then(|rest| rest.strip_suffix(&closing))
        {
            return Some((level, unescape_html(text)));
        }
    }

    None
}

fn parse_html_paragraph(line: &str) -> Option<String> {
    line.strip_prefix("<p>")
        .and_then(|rest| rest.strip_suffix("</p>"))
        .map(unescape_html)
}

fn push_markdown_block(markdown: &mut String, block: &str) {
    if !markdown.is_empty() {
        markdown.push_str("\n\n");
    }
    markdown.push_str(block);
}

#[derive(Debug, PartialEq, Eq)]
enum MarkdownBlock {
    Heading { level: usize, text: String },
    Paragraph(String),
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
        add_note, collect_page_paths, escape_html, export_page, html_to_markdown,
        import_markdown, insert_note_into_document, markdown_to_html, new_page,
        note_id_from_trigger, patch_note, remove_note, render_note_aside, render_page_document,
        resolve_page_destination, stylesheet_href, sync_project, validate_page_metadata,
        validate_project, FileEntry, GraphPageLink, LinkEntry, NoteEntry, PageDocument,
        PageEntry, ProjectGraph, ProjectIndex, ProjectManifest, Theme,
    };
    use std::collections::BTreeMap;
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

    fn required_meta() -> BTreeMap<String, String> {
        [
            (
                "fractal:summary".to_string(),
                "Short page summary here.".to_string(),
            ),
            (
                "fractal:tags".to_string(),
                "rust, graphs, parsing".to_string(),
            ),
            ("fractal:version".to_string(), "0.1".to_string()),
        ]
        .into_iter()
        .collect()
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
    fn markdown_import_converts_basic_blocks_to_html() {
        let (title, body) = markdown_to_html(
            "fallback",
            "# Title\n\nIntro line\ncontinued line\n\n## Section\n\nBody & <more>",
        );

        assert_eq!(title, "Title");
        assert_eq!(
            body,
            "<p>Intro line continued line</p>\n      <h2>Section</h2>\n      <p>Body &amp; &lt;more&gt;</p>"
        );
    }

    #[test]
    fn markdown_import_keeps_non_initial_h1_in_body() {
        let (title, body) = markdown_to_html("fallback", "Intro\n\n# Later");

        assert_eq!(title, "fallback");
        assert_eq!(body, "<p>Intro</p>\n      <h1>Later</h1>");
    }

    #[test]
    fn html_export_converts_basic_blocks_to_markdown() {
        let html = render_page_document(
            "Title",
            "<p>Intro &amp; more</p>\n      <h2>Section</h2>\n      <p>Body</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        );

        assert_eq!(
            html_to_markdown(&html),
            "# Title\n\nIntro & more\n\n## Section\n\nBody"
        );
    }

    #[test]
    fn import_and_export_markdown_files() {
        let root = temp_dir("markdown-io");
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
        let source = root.join("source.md");
        fs::write(&source, "# Source\n\nHello\n\n### Deep").expect("write markdown");

        import_markdown(&root, &source).expect("import markdown");
        let page = fs::read_to_string(pages_dir.join("source.html")).expect("read imported page");
        assert!(page.contains("<h1>Source</h1>"));
        assert!(page.contains("<p>Hello</p>"));
        assert!(page.contains("<h3>Deep</h3>"));

        let output = root.join("out.md");
        export_page(&root, Path::new("pages/source.html"), &output).expect("export markdown");
        assert_eq!(
            fs::read_to_string(output).expect("read exported markdown"),
            "# Source\n\nHello\n\n### Deep"
        );

        fs::remove_dir_all(&root).expect("cleanup temp dir");
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
    fn validate_ignores_non_html_files_under_pages() {
        let root = temp_dir("validate-assets");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(&pages_dir).expect("create pages dir");
        fs::create_dir_all(&workspace_dir).expect("create workspace dir");
        fs::write(workspace_dir.join("style.css"), "").expect("write stylesheet");
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
        fs::write(pages_dir.join("asset.txt"), "asset").expect("write asset");

        validate_project(&root, false).expect("validate project");

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
            index.files,
            vec![
                FileEntry {
                    path: "index.html".to_string(),
                    kind: "page".to_string(),
                },
                FileEntry {
                    path: "secondpage.html".to_string(),
                    kind: "page".to_string(),
                },
            ]
        );
        assert_eq!(
            index.pages,
            vec![
                PageEntry {
                    path: "index.html".to_string(),
                    title: "index".to_string(),
                    meta: required_meta(),
                    notes: vec![],
                    links: vec![],
                },
                PageEntry {
                    path: "secondpage.html".to_string(),
                    title: "secondpage".to_string(),
                    meta: required_meta(),
                    notes: vec![],
                    links: vec![],
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

        let meta = PageDocument::from_path(&page)
            .expect("parse page")
            .fractal_meta();

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
    fn page_document_extracts_from_ordinary_html_formatting() {
        let document = PageDocument::parse(
            r#"<!doctype html>
<html>
  <head>
    <META NAME='fractal:version' CONTENT='0.1'>
    <meta content='Summary &amp; detail' name='fractal:summary'>
    <title>Flexible   Page</title>
  </head>
  <body>
    <main>
      <p><a HREF='topic.html'><strong>Topic</strong></a></p>
    </main>
    <section data-fractal-notes>
      <aside data-fractal-note id='note-java' data-fractal-trigger='Java language'></aside>
    </section>
  </body>
</html>"#,
        );

        assert!(document.has_notes_section());
        assert_eq!(document.title(), Some("Flexible Page".to_string()));
        assert_eq!(
            document.fractal_meta(),
            [
                ("fractal:summary".to_string(), "Summary & detail".to_string()),
                ("fractal:version".to_string(), "0.1".to_string()),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            document.notes(),
            vec![NoteEntry {
                id: "note-java".to_string(),
                label: "Java language".to_string(),
            }]
        );
        assert_eq!(
            document.links(),
            vec![LinkEntry {
                href: "topic.html".to_string(),
                text: "Topic".to_string(),
                scope: "page".to_string(),
            }]
        );
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

    #[test]
    fn sync_rebuilds_index_and_links_notes_before_project_pages() {
        let root = temp_dir("sync-links");
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

        let index = render_page_document(
            "Home",
            "<p>Java and Rust are mentioned here.</p>",
            Theme::Dark,
            "../.fractal/style.css".to_string(),
        );
        let index = insert_note_into_document(&index, &render_note_aside("note-java", "note body"))
            .expect("insert note");
        fs::write(pages_dir.join("index.html"), index).expect("write index page");
        fs::write(
            pages_dir.join("rust.html"),
            render_page_document(
                "Rust",
                "<p>Home mentions Java.</p>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write rust page");
        fs::write(pages_dir.join("asset.txt"), "asset").expect("write asset");

        sync_project(&root).expect("sync project");
        sync_project(&root).expect("sync project twice");

        let html = fs::read_to_string(pages_dir.join("index.html")).expect("read index page");
        assert_eq!(html.matches("data-fractal-link=\"note\"").count(), 1);
        assert_eq!(html.matches("data-fractal-link=\"page\"").count(), 1);
        assert!(html.contains("<a href=\"#note-java\" data-fractal-link=\"note\">Java</a>"));
        assert!(html.contains("<a href=\"rust.html\" data-fractal-link=\"page\">Rust</a>"));

        let index: ProjectIndex = serde_json::from_str(
            &fs::read_to_string(workspace_dir.join("index.json")).expect("read index"),
        )
        .expect("parse index");
        assert_eq!(
            index.files,
            vec![
                FileEntry {
                    path: "asset.txt".to_string(),
                    kind: "asset".to_string(),
                },
                FileEntry {
                    path: "index.html".to_string(),
                    kind: "page".to_string(),
                },
                FileEntry {
                    path: "rust.html".to_string(),
                    kind: "page".to_string(),
                },
            ]
        );
        assert_eq!(
            index.pages[0].notes,
            vec![NoteEntry {
                id: "note-java".to_string(),
                label: "java".to_string(),
            }]
        );
        assert_eq!(
            index.pages[0].links,
            vec![
                LinkEntry {
                    href: "#note-java".to_string(),
                    text: "Java".to_string(),
                    scope: "note".to_string(),
                },
                LinkEntry {
                    href: "rust.html".to_string(),
                    text: "Rust".to_string(),
                    scope: "page".to_string(),
                },
            ]
        );

        let graph: ProjectGraph = serde_json::from_str(
            &fs::read_to_string(workspace_dir.join("graph.json")).expect("read graph"),
        )
        .expect("parse graph");
        assert!(graph.nodes.iter().any(|node| node.id == "page:index.html"));
        assert!(graph.nodes.iter().any(|node| node.id == "page:rust.html"));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.id == "note:index.html#note-java"));
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "page:index.html"
                && edge.to == "note:index.html#note-java"
                && edge.kind == "links_to_note"
        }));

        let index_page_graph = graph
            .pages
            .iter()
            .find(|page| page.path == "index.html")
            .expect("index page graph");
        assert_eq!(
            index_page_graph.outlinks,
            vec![GraphPageLink {
                page: "rust.html".to_string(),
                text: "Rust".to_string(),
            }]
        );
        assert_eq!(
            index_page_graph.backlinks,
            vec![GraphPageLink {
                page: "rust.html".to_string(),
                text: "Home".to_string(),
            }]
        );

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }

    #[test]
    fn sync_uses_relative_links_from_nested_pages() {
        let root = temp_dir("sync-nested-links");
        let pages_dir = root.join("pages");
        let workspace_dir = root.join(".fractal");
        fs::create_dir_all(pages_dir.join("folder")).expect("create pages dir");
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
                "Home",
                "<p>Nested Topic lives elsewhere.</p>",
                Theme::Dark,
                "../.fractal/style.css".to_string(),
            ),
        )
        .expect("write index page");
        fs::write(
            pages_dir.join("folder/topic.html"),
            render_page_document(
                "Nested Topic",
                "<p>Home is the default page.</p>",
                Theme::Dark,
                "../../.fractal/style.css".to_string(),
            ),
        )
        .expect("write nested page");

        sync_project(&root).expect("sync project");

        let nested =
            fs::read_to_string(pages_dir.join("folder/topic.html")).expect("read nested page");
        assert!(nested.contains("<a href=\"../index.html\" data-fractal-link=\"page\">Home</a>"));

        fs::remove_dir_all(&root).expect("cleanup temp dir");
    }
}
