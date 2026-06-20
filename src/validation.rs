use crate::document::notes::is_valid_note_id;
use crate::document::render::{
    default_stylesheet, render_page_document, required_meta_tags, stylesheet_href,
};
use crate::document::PageDocument;
use crate::graph::links::{
    is_external_href, link_label_key, normalize_link_label, page_link_labels,
    page_link_text_matches, resolve_page_href,
};
use crate::index::ensure_page_labels_available_for;
use crate::io::fs::atomic_write;
use crate::project::constants::{INDEX_PAGE, MANIFEST_FILE, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR};
use crate::project::paths::{collect_page_paths, is_html_path, load_manifest};
use crate::types::{OperationEvent, OperationReport, Theme};
use crate::{FractalError, Result};
use kuchiki::NodeRef;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub fn validate_project(root: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut report = OperationReport::new();

    let manifest_path = root.join(MANIFEST_FILE);
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);
    let manifest = load_manifest(root)?;

    if !workspace_dir.is_dir() {
        return Err(FractalError::invalid_project(format!(
            "missing workspace directory: {}",
            workspace_dir.display()
        )));
    }

    let stylesheet = workspace_dir.join(STYLE_FILE);
    if !stylesheet.is_file() {
        return Err(FractalError::invalid_project(format!(
            "missing stylesheet: {}",
            stylesheet.display()
        )));
    }

    if !pages_dir.is_dir() {
        return Err(FractalError::invalid_project(format!(
            "missing pages directory: {}",
            pages_dir.display()
        )));
    }

    if !manifest.default_page.trim().is_empty() {
        let default_page = root.join(&manifest.default_page);
        if !default_page.is_file() {
            return Err(FractalError::invalid_project(format!(
                "missing default page: {}",
                default_page.display()
            )));
        }
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();
    let known_page_paths = known_html_page_paths(&page_paths);
    let known_page_titles = known_page_titles(&pages_dir, &page_paths)?;

    for page_path in &page_paths {
        if !is_html_path(page_path) {
            continue;
        }

        let page = pages_dir.join(page_path);
        validate_project_page(
            &page,
            page_path,
            manifest.theme,
            &known_page_paths,
            &known_page_titles,
        )?;
    }

    report.extend(warn_duplicate_page_labels(&known_page_titles));

    report.push(OperationEvent::ProjectValidated {
        project_name: manifest.project_name,
        manifest_path,
    });
    Ok(report.relative_to(root))
}

pub fn repair_project(root: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    let mut report = fix_project(root, RepairMode::Write)?;
    report.extend(validate_project(root)?);
    Ok(report.relative_to(root))
}

pub fn preflight_repair_project(root: impl AsRef<Path>) -> Result<OperationReport> {
    let root = root.as_ref();
    Ok(fix_project(root, RepairMode::DryRun)?.relative_to(root))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepairMode {
    DryRun,
    Write,
}

impl RepairMode {
    fn writes(self) -> bool {
        matches!(self, Self::Write)
    }
}

fn fix_project(root: &Path, mode: RepairMode) -> Result<OperationReport> {
    let manifest = load_manifest(root)?;
    let workspace_dir = root.join(WORKSPACE_DIR);
    let pages_dir = root.join(PAGES_DIR);
    let mut report = OperationReport::new();

    if mode.writes() {
        fs::create_dir_all(&workspace_dir)?;
        fs::create_dir_all(&pages_dir)?;
    }

    let stylesheet = workspace_dir.join(STYLE_FILE);
    if !stylesheet.is_file() {
        if mode.writes() {
            atomic_write(&stylesheet, default_stylesheet())?;
        }
        report.push(OperationEvent::ProjectRepaired {
            path: stylesheet,
            applied: mode.writes(),
        });
    }

    if !manifest.default_page.trim().is_empty() {
        let default_page = root.join(&manifest.default_page);
        if !default_page.is_file() {
            if mode.writes() {
                if let Some(parent) = default_page.parent() {
                    fs::create_dir_all(parent)?;
                }
            }

            let title = default_page
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(&manifest.project_name);
            let page_path = default_page
                .strip_prefix(&pages_dir)
                .unwrap_or_else(|_| Path::new(INDEX_PAGE));
            if mode.writes() {
                atomic_write(
                    &default_page,
                    render_page_document(title, "", manifest.theme, stylesheet_href(page_path)),
                )?;
            }
            report.push(OperationEvent::ProjectRepaired {
                path: default_page.clone(),
                applied: mode.writes(),
            });
        }
    }

    if !pages_dir.is_dir() {
        return Ok(report.relative_to(root));
    }

    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    page_paths.sort();
    let known_page_titles = known_page_titles(&pages_dir, &page_paths)?;

    for page_path in page_paths {
        if !is_html_path(&page_path) {
            continue;
        }

        let page = pages_dir.join(&page_path);
        if fix_page(&page, &page_path, manifest.theme, &known_page_titles, mode)? {
            report.push(OperationEvent::ProjectRepaired {
                path: page,
                applied: mode.writes(),
            });
        }
    }

    Ok(report.relative_to(root))
}

#[cfg(test)]
pub(crate) fn validate_page_metadata(page: &Path) -> Result<()> {
    let html = fs::read_to_string(page)?;
    let document = PageDocument::parse(&html);
    validate_page_structure(page, "page.html", Theme::Dark, &document)?;
    validate_note_ids(page, &document)?;
    Ok(())
}

pub(crate) fn validate_page_html_for_project(
    root: &Path,
    page_path: &str,
    html: &str,
) -> Result<()> {
    let manifest = load_manifest(root)?;
    let pages_dir = root.join(PAGES_DIR);
    let page_paths = page_paths_for_candidate(root, page_path)?;
    let known_page_paths = known_html_page_paths(&page_paths);
    let document = PageDocument::parse(html);
    let display_path = pages_dir.join(page_path);
    let known_page_titles = known_page_titles_for_candidate(root, page_path, &document)?;

    validate_page_structure(&display_path, page_path, manifest.theme, &document)?;
    validate_note_ids(&display_path, &document)?;
    validate_generated_links(
        &display_path,
        page_path,
        &document,
        &known_page_paths,
        &known_page_titles,
    )?;

    let title = document.title().ok_or_else(|| {
        FractalError::invalid_project(format!("missing page title in {}", display_path.display()))
    })?;
    ensure_page_labels_available_for(root, Some(page_path), page_path, &title)?;

    Ok(())
}

fn warn_duplicate_page_labels(known_page_titles: &BTreeMap<String, String>) -> OperationReport {
    let mut report = OperationReport::new();
    let mut owners = BTreeMap::<String, String>::new();

    for (path, title) in known_page_titles {
        for label in page_link_labels(path, title) {
            let label = normalize_link_label(&label);
            if label.is_empty() {
                continue;
            }
            let key = link_label_key(&label);
            if let Some(existing_path) = owners.get(&key) {
                if existing_path != path {
                    report.push(OperationEvent::Warning {
                        message: format!(
                            "duplicate page label `{label}` for {existing_path} and {path}; behavior is undefined"
                        ),
                    });
                }
            } else {
                owners.insert(key, path.clone());
            }
        }
    }

    report
}

fn validate_project_page(
    page: &Path,
    page_path: &str,
    theme: Theme,
    known_page_paths: &BTreeSet<String>,
    known_page_titles: &BTreeMap<String, String>,
) -> Result<()> {
    let document = PageDocument::from_path(page)?;
    validate_page_structure(page, page_path, theme, &document)?;
    validate_note_ids(page, &document)?;
    validate_generated_links(
        page,
        page_path,
        &document,
        known_page_paths,
        known_page_titles,
    )?;
    Ok(())
}

fn fix_page(
    page: &Path,
    page_path: &str,
    theme: Theme,
    known_page_titles: &BTreeMap<String, String>,
    mode: RepairMode,
) -> Result<bool> {
    let document = PageDocument::from_path(page)?;
    let mut changed = false;

    for (name, content) in required_meta_tags() {
        if document.ensure_meta_tag(name, content)? {
            changed = true;
        }
    }

    if document.set_stylesheet_href(&stylesheet_href(Path::new(page_path)))? {
        changed = true;
    }

    if document.ensure_body_theme(theme.as_str())? {
        changed = true;
    }

    if document.ensure_single_notes_section()? {
        changed = true;
    }

    if document.repair_invalid_links(page_path, known_page_titles) > 0 {
        changed = true;
    }

    if fix_missing_title_or_heading(&document, page_path)? {
        changed = true;
    }

    if changed && mode.writes() {
        atomic_write(page, document.to_html()?)?;
    }

    Ok(changed)
}

fn fix_missing_title_or_heading(document: &PageDocument, page_path: &str) -> Result<bool> {
    let title_count = select_nodes(document, "title").len();
    let main_h1_count = select_nodes(document, "main > h1").len();
    if title_count > 0 && main_h1_count > 0 {
        return Ok(false);
    }

    let title = document
        .element_text("title")
        .or_else(|| document.element_text("main > h1"))
        .map(|title| normalize_link_label(&title))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| page_title_from_path(page_path));

    document.set_title(&title)
}

fn validate_page_structure(
    page: &Path,
    page_path: &str,
    theme: Theme,
    document: &PageDocument,
) -> Result<()> {
    let head = single_node(page, document, "head")?;
    let body = single_node(page, document, "body")?;
    let main = single_node(page, document, "main")?;
    let notes_section = single_node(page, document, "section[data-fractal-notes]")?;

    validate_head_children(page, &head)?;
    validate_body_children(page, &body)?;
    validate_title_contract(page, document, &main)?;
    validate_required_meta(page, document)?;
    validate_stylesheet(page, page_path, &head)?;
    validate_body_theme(page, theme, &body)?;
    validate_notes_section(page, &notes_section)?;
    validate_allowed_body_content(page, &main, true)?;

    Ok(())
}

fn validate_head_children(page: &Path, head: &NodeRef) -> Result<()> {
    for child in meaningful_children(head) {
        let Some(element) = child.as_element() else {
            return Err(FractalError::invalid_project(format!(
                "invalid direct head child in {}: {}",
                page.display(),
                node_label(&child)
            )));
        };
        let name = element.name.local.to_string();
        match name.as_str() {
            "title" | "meta" => {}
            "link" => {
                let attributes = element.attributes.borrow();
                let rel = attributes.get("rel").unwrap_or_default();
                if !rel
                    .split_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                {
                    return Err(FractalError::invalid_project(format!(
                        "unsupported Fractal head link in {}: rel={rel}",
                        page.display()
                    )));
                }
            }
            _ => {
                return Err(FractalError::invalid_project(format!(
                    "unsupported Fractal head element in {}: <{name}>",
                    page.display()
                )));
            }
        }
    }

    Ok(())
}

fn validate_body_children(page: &Path, body: &NodeRef) -> Result<()> {
    let mut main_count = 0;
    let mut notes_count = 0;

    for child in meaningful_children(body) {
        if is_element_named(&child, "main") {
            main_count += 1;
            continue;
        }
        if is_notes_section(&child) {
            notes_count += 1;
            continue;
        }

        return Err(FractalError::invalid_project(format!(
            "invalid direct body child in {}: {}",
            page.display(),
            node_label(&child)
        )));
    }

    if main_count != 1 {
        return Err(FractalError::invalid_project(format!(
            "expected exactly one direct main child in {}: found {main_count}",
            page.display()
        )));
    }
    if notes_count != 1 {
        return Err(FractalError::invalid_project(format!(
            "expected exactly one direct notes section in {}: found {notes_count}",
            page.display()
        )));
    }

    Ok(())
}

fn validate_title_contract(page: &Path, document: &PageDocument, main: &NodeRef) -> Result<()> {
    let title = single_node(page, document, "title")?;
    let title_text = normalize_link_label(&title.text_contents());
    if title_text.is_empty() {
        return Err(FractalError::invalid_project(format!(
            "page title cannot be empty in {}",
            page.display()
        )));
    }

    let children = meaningful_children(main);
    let Some(first_child) = children.first() else {
        return Err(FractalError::invalid_project(format!(
            "missing main heading in {}",
            page.display()
        )));
    };
    if !is_element_named(first_child, "h1") {
        return Err(FractalError::invalid_project(format!(
            "main heading must be the first meaningful child in {}",
            page.display()
        )));
    }

    let h1_count = select_nodes(document, "main h1").len();
    if h1_count != 1 {
        return Err(FractalError::invalid_project(format!(
            "expected exactly one main h1 in {}: found {h1_count}",
            page.display()
        )));
    }

    let heading_text = normalize_link_label(&first_child.text_contents());
    if heading_text.is_empty() {
        return Err(FractalError::invalid_project(format!(
            "main heading cannot be empty in {}",
            page.display()
        )));
    }
    if title_text != heading_text {
        return Err(FractalError::invalid_project(format!(
            "title and main heading differ in {}: `{title_text}` != `{heading_text}`",
            page.display()
        )));
    }

    Ok(())
}

fn validate_required_meta(page: &Path, document: &PageDocument) -> Result<()> {
    let allowed = required_meta_tags().into_iter().collect::<BTreeMap<_, _>>();
    let mut found = BTreeMap::<String, Vec<String>>::new();

    for element in document
        .document
        .select("meta[name]")
        .expect("static selector should parse")
    {
        let attributes = element.attributes.borrow();
        let Some(name) = attributes.get("name") else {
            continue;
        };
        if !name.starts_with("fractal:") {
            continue;
        }
        if !allowed.contains_key(name) {
            return Err(FractalError::invalid_project(format!(
                "unsupported Fractal meta tag in {}: {name}",
                page.display()
            )));
        }

        let Some(content) = attributes.get("content") else {
            return Err(FractalError::invalid_project(format!(
                "missing content for required meta tag in {}: {name}",
                page.display()
            )));
        };
        found
            .entry(name.to_string())
            .or_default()
            .push(content.to_string());
    }

    for (name, expected_content) in allowed {
        let Some(values) = found.get(name) else {
            return Err(FractalError::invalid_project(format!(
                "missing required meta tag in {}: {name}",
                page.display()
            )));
        };
        if values.len() != 1 {
            return Err(FractalError::invalid_project(format!(
                "duplicate required meta tag in {}: {name}",
                page.display()
            )));
        }
        if name == "fractal:version" && values[0] != expected_content {
            return Err(FractalError::invalid_project(format!(
                "unsupported page format version in {}: {} (expected {})",
                page.display(),
                values[0],
                expected_content
            )));
        }
    }

    Ok(())
}

fn validate_stylesheet(page: &Path, page_path: &str, head: &NodeRef) -> Result<()> {
    let expected = stylesheet_href(Path::new(page_path));
    let has_expected = head
        .select("link[href][rel]")
        .expect("static selector should parse")
        .any(|element| {
            let attributes = element.attributes.borrow();
            let rel = attributes.get("rel").unwrap_or_default();
            rel.split_whitespace()
                .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                && attributes.get("href") == Some(expected.as_str())
        });

    if !has_expected {
        return Err(FractalError::invalid_project(format!(
            "missing exact Fractal stylesheet link in {}: expected {expected}",
            page.display()
        )));
    }

    Ok(())
}

fn validate_body_theme(page: &Path, theme: Theme, body: &NodeRef) -> Result<()> {
    let Some(element) = body.as_element() else {
        return Err(FractalError::invalid_project(format!(
            "body node is not an element in {}",
            page.display()
        )));
    };
    let attributes = element.attributes.borrow();
    let expected = theme.as_str();
    if attributes.get("data-fractal-theme") != Some(expected) {
        return Err(FractalError::invalid_project(format!(
            "body theme mismatch in {}: expected {expected}",
            page.display()
        )));
    }

    Ok(())
}

fn validate_notes_section(page: &Path, notes_section: &NodeRef) -> Result<()> {
    for child in meaningful_children(notes_section) {
        if !is_fractal_note_aside(&child) {
            return Err(FractalError::invalid_project(format!(
                "invalid notes section child in {}: {}",
                page.display(),
                node_label(&child)
            )));
        }
        validate_allowed_body_content(page, &child, false)?;
    }

    Ok(())
}

fn validate_note_ids(page: &Path, document: &PageDocument) -> Result<()> {
    let notes_section = single_node(page, document, "section[data-fractal-notes]")?;
    let mut seen = BTreeSet::new();

    for element in document
        .document
        .select("aside[data-fractal-note]")
        .expect("static selector should parse")
    {
        let node = element.as_node().clone();
        if node.parent() != Some(notes_section.clone()) {
            return Err(FractalError::invalid_project(format!(
                "note must be a direct child of the notes section in {}",
                page.display()
            )));
        }

        let attributes = element.attributes.borrow();
        let Some(id) = attributes.get("id") else {
            return Err(FractalError::invalid_project(format!(
                "missing note id in {}",
                page.display()
            )));
        };

        if !is_valid_note_id(id) {
            return Err(FractalError::invalid_project(format!(
                "malformed note id in {}: {id}",
                page.display()
            )));
        }

        if !seen.insert(id.to_string()) {
            return Err(FractalError::invalid_project(format!(
                "duplicate note id in {}: {id}",
                page.display()
            )));
        }
    }

    Ok(())
}

fn validate_allowed_body_content(page: &Path, root: &NodeRef, requires_h1: bool) -> Result<()> {
    validate_direct_content_children(page, root, requires_h1)?;
    let first_h1 = requires_h1
        .then(|| meaningful_children(root).into_iter().next())
        .flatten();

    for node in root.descendants() {
        if node == *root {
            continue;
        }
        let Some(element) = node.as_element() else {
            continue;
        };
        let name = element.name.local.to_string();
        if first_h1.as_ref() == Some(&node) {
            continue;
        }
        if name == "h1" {
            return Err(FractalError::invalid_project(format!(
                "unexpected h1 in {}",
                page.display()
            )));
        }
        if !is_allowed_body_element(&name) {
            return Err(FractalError::invalid_project(format!(
                "unsupported Fractal body element in {}: <{name}>",
                page.display()
            )));
        }
    }

    validate_list_children(page, root)?;
    Ok(())
}

fn validate_direct_content_children(page: &Path, root: &NodeRef, requires_h1: bool) -> Result<()> {
    let children = meaningful_children(root);
    let start = if requires_h1 { 1 } else { 0 };

    if requires_h1
        && !children
            .first()
            .map(|node| is_element_named(node, "h1"))
            .unwrap_or(false)
    {
        return Err(FractalError::invalid_project(format!(
            "missing first main h1 in {}",
            page.display()
        )));
    }

    for child in children.into_iter().skip(start) {
        let Some(element) = child.as_element() else {
            return Err(FractalError::invalid_project(format!(
                "unsupported direct content node in {}: {}",
                page.display(),
                node_label(&child)
            )));
        };
        let name = element.name.local.to_string();
        if !is_allowed_body_element(&name) {
            return Err(FractalError::invalid_project(format!(
                "unsupported direct Fractal body element in {}: <{name}>",
                page.display()
            )));
        }
    }

    Ok(())
}

fn validate_list_children(page: &Path, root: &NodeRef) -> Result<()> {
    for list in root
        .select("ul, ol")
        .expect("static selector should parse")
        .map(|element| element.as_node().clone())
    {
        for child in meaningful_children(&list) {
            if !is_element_named(&child, "li") {
                return Err(FractalError::invalid_project(format!(
                    "list children must be li elements in {}: {}",
                    page.display(),
                    node_label(&child)
                )));
            }
        }
    }

    Ok(())
}

fn validate_generated_links(
    page: &Path,
    page_path: &str,
    document: &PageDocument,
    known_page_paths: &BTreeSet<String>,
    known_page_titles: &BTreeMap<String, String>,
) -> Result<()> {
    let note_ids = document
        .notes()
        .into_iter()
        .map(|note| note.id)
        .collect::<BTreeSet<_>>();

    for element in document
        .document
        .select("a")
        .expect("static selector should parse")
    {
        let attributes = element.attributes.borrow();
        let scope = attributes.get("data-fractal-link");
        let Some(href) = attributes.get("href") else {
            return Err(FractalError::invalid_project(format!(
                "generated link is missing href in {}",
                page.display()
            )));
        };
        let text = normalize_link_label(&element.text_contents());

        let Some(scope) = scope else {
            diagnose_manual_link_target(page, page_path, href, &text, known_page_titles)?;
            return Err(FractalError::invalid_project(format!(
                "manual link is not valid Fractal in {}",
                page.display()
            )));
        };

        match scope {
            "page" => {
                let Some(target) = resolve_page_href(page_path, href) else {
                    return Err(FractalError::invalid_project(format!(
                        "generated page link does not resolve in {}: {href}",
                        page.display()
                    )));
                };
                if !known_page_paths.contains(&target) {
                    return Err(FractalError::invalid_project(format!(
                        "generated page link target is missing in {}: {href}",
                        page.display()
                    )));
                }
                validate_page_link_text(
                    page,
                    "generated",
                    href,
                    &text,
                    &target,
                    known_page_titles,
                )?;
            }
            "note" => {
                let Some(note_id) = href.strip_prefix('#') else {
                    return Err(FractalError::invalid_project(format!(
                        "generated note link must target a page-local note in {}: {href}",
                        page.display()
                    )));
                };
                if !note_ids.contains(note_id) {
                    return Err(FractalError::invalid_project(format!(
                        "generated note link target is missing in {}: {href}",
                        page.display()
                    )));
                }
            }
            _ => {
                return Err(FractalError::invalid_project(format!(
                    "unsupported generated link scope in {}: {scope}",
                    page.display()
                )));
            }
        }
    }

    Ok(())
}

fn diagnose_manual_link_target(
    page: &Path,
    page_path: &str,
    href: &str,
    text: &str,
    known_page_titles: &BTreeMap<String, String>,
) -> Result<()> {
    if href.starts_with('#') || is_external_href(href) {
        return Ok(());
    }

    let Some(target) = resolve_page_href(page_path, href) else {
        return Ok(());
    };
    if !known_page_titles.contains_key(&target) {
        return Ok(());
    }

    validate_page_link_text(page, "manual", href, text, &target, known_page_titles)
}

fn validate_page_link_text(
    page: &Path,
    kind: &str,
    href: &str,
    text: &str,
    target: &str,
    known_page_titles: &BTreeMap<String, String>,
) -> Result<()> {
    let Some(title) = known_page_titles.get(target) else {
        return Ok(());
    };
    if page_link_text_matches(target, title, text) {
        return Ok(());
    }

    Err(FractalError::invalid_project(format!(
        "{kind} page link text does not identify its target in {}: `{}` -> {href} (expected {})",
        page.display(),
        text,
        expected_page_labels(target, title)
    )))
}

fn expected_page_labels(path: &str, title: &str) -> String {
    let mut labels = Vec::new();
    let mut seen = BTreeSet::new();
    for label in page_link_labels(path, title) {
        let label = normalize_link_label(&label);
        if label.is_empty() {
            continue;
        }
        if seen.insert(link_label_key(&label)) {
            labels.push(format!("`{label}`"));
        }
    }
    labels.join(" or ")
}

fn single_node(page: &Path, document: &PageDocument, selector: &str) -> Result<NodeRef> {
    let nodes = select_nodes(document, selector);
    let label = match selector {
        "section[data-fractal-notes]" => "notes section",
        "main" => "main section",
        "title" => "page title",
        _ => selector,
    };

    match nodes.as_slice() {
        [node] => Ok(node.clone()),
        [] => Err(FractalError::invalid_project(format!(
            "missing {label} in {}",
            page.display()
        ))),
        _ => Err(FractalError::invalid_project(format!(
            "duplicate {label} in {}: {} found",
            page.display(),
            nodes.len()
        ))),
    }
}

fn select_nodes(document: &PageDocument, selector: &str) -> Vec<NodeRef> {
    document
        .document
        .select(selector)
        .expect("static selector should parse")
        .map(|element| element.as_node().clone())
        .collect()
}

pub(crate) fn known_page_titles_for_candidate(
    root: &Path,
    page_path: &str,
    document: &PageDocument,
) -> Result<BTreeMap<String, String>> {
    let pages_dir = root.join(PAGES_DIR);
    let page_paths = page_paths_for_candidate(root, page_path)?;
    let mut titles = known_page_titles(&pages_dir, &page_paths)?;

    titles.insert(
        page_path.to_string(),
        document
            .title()
            .unwrap_or_else(|| page_title_from_path(page_path)),
    );

    Ok(titles)
}

fn page_paths_for_candidate(root: &Path, page_path: &str) -> Result<Vec<String>> {
    let pages_dir = root.join(PAGES_DIR);
    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    if !page_paths.iter().any(|path| path == page_path) {
        page_paths.push(page_path.to_string());
    }
    page_paths.sort();
    Ok(page_paths)
}

fn known_html_page_paths(page_paths: &[String]) -> BTreeSet<String> {
    page_paths
        .iter()
        .filter(|path| is_html_path(path))
        .cloned()
        .collect()
}

fn known_page_titles(pages_dir: &Path, page_paths: &[String]) -> Result<BTreeMap<String, String>> {
    let mut titles = BTreeMap::new();

    for page_path in page_paths.iter().filter(|path| is_html_path(path)) {
        let page = pages_dir.join(page_path);
        if !page.is_file() {
            continue;
        }

        let document = PageDocument::from_path(&page)?;
        let title = document
            .title()
            .unwrap_or_else(|| page_title_from_path(page_path));
        titles.insert(page_path.clone(), title);
    }

    Ok(titles)
}

fn meaningful_children(node: &NodeRef) -> Vec<NodeRef> {
    node.children()
        .filter(|child| !is_blank_text(child))
        .collect()
}

fn is_blank_text(node: &NodeRef) -> bool {
    node.as_text()
        .map(|text| text.borrow().trim().is_empty())
        .unwrap_or(false)
}

fn is_element_named(node: &NodeRef, name: &str) -> bool {
    node.as_element()
        .map(|element| element.name.local.to_string() == name)
        .unwrap_or(false)
}

fn is_notes_section(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    element.name.local.to_string() == "section"
        && element.attributes.borrow().contains("data-fractal-notes")
}

fn is_fractal_note_aside(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    element.name.local.to_string() == "aside"
        && element.attributes.borrow().contains("data-fractal-note")
}

fn is_allowed_body_element(name: &str) -> bool {
    matches!(
        name,
        "p" | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "blockquote"
            | "pre"
            | "code"
            | "a"
    )
}

fn node_label(node: &NodeRef) -> String {
    if let Some(element) = node.as_element() {
        format!("<{}>", element.name.local)
    } else if node.as_text().is_some() {
        "text".to_string()
    } else {
        "non-element node".to_string()
    }
}

fn page_title_from_path(page_path: &str) -> String {
    Path::new(page_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| normalize_link_label(&stem.replace(['-', '_'], " ")))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| page_path.to_string())
}
