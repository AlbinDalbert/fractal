use crate::document::notes::is_valid_note_id;
use crate::document::render::{
    default_stylesheet, render_page_document, required_meta_tags, stylesheet_href,
};
use crate::document::PageDocument;
use crate::graph::links::{normalize_link_label, resolve_page_href};
use crate::index::{build_project_index, ensure_page_labels_available_for};
use crate::project::constants::{INDEX_PAGE, MANIFEST_FILE, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR};
use crate::project::paths::{collect_page_paths, is_html_path, load_manifest};
use crate::types::{OperationEvent, OperationReport, Theme};
use crate::Result;
use kuchiki::NodeRef;
use std::collections::{BTreeMap, BTreeSet};
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
    let known_page_paths = known_html_page_paths(&page_paths);

    for page_path in &page_paths {
        if !is_html_path(page_path) {
            continue;
        }

        let page = pages_dir.join(page_path);
        validate_project_page(&page, page_path, manifest.theme, &known_page_paths)?;
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
    let mut page_paths = Vec::new();
    collect_page_paths(&pages_dir, &pages_dir, &mut page_paths)?;
    if !page_paths.iter().any(|path| path == page_path) {
        page_paths.push(page_path.to_string());
    }
    page_paths.sort();
    let known_page_paths = known_html_page_paths(&page_paths);
    let document = PageDocument::parse(html);
    let display_path = pages_dir.join(page_path);

    validate_page_structure(&display_path, page_path, manifest.theme, &document)?;
    validate_note_ids(&display_path, &document)?;
    validate_generated_links(&display_path, page_path, &document, &known_page_paths)?;

    let title = document
        .title()
        .ok_or_else(|| format!("missing page title in {}", display_path.display()))?;
    ensure_page_labels_available_for(root, Some(page_path), page_path, &title)?;

    Ok(())
}

fn validate_project_page(
    page: &Path,
    page_path: &str,
    theme: Theme,
    known_page_paths: &BTreeSet<String>,
) -> Result<()> {
    let document = PageDocument::from_path(page)?;
    validate_page_structure(page, page_path, theme, &document)?;
    validate_note_ids(page, &document)?;
    validate_generated_links(page, page_path, &document, known_page_paths)?;
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

    if document.set_stylesheet_href(&stylesheet_href(Path::new(page_path)))? {
        changed = true;
    }

    if document.ensure_body_theme(theme.as_str())? {
        changed = true;
    }

    if document.ensure_single_notes_section()? {
        changed = true;
    }

    if document.unwrap_manual_links() > 0 {
        changed = true;
    }

    if fix_missing_title_or_heading(&document, page_path)? {
        changed = true;
    }

    if changed {
        fs::write(page, document.to_html()?)?;
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
            return Err(format!(
                "invalid direct head child in {}: {}",
                page.display(),
                node_label(&child)
            )
            .into());
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
                    return Err(format!(
                        "unsupported Fractal head link in {}: rel={rel}",
                        page.display()
                    )
                    .into());
                }
            }
            _ => {
                return Err(format!(
                    "unsupported Fractal head element in {}: <{name}>",
                    page.display()
                )
                .into())
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

        return Err(format!(
            "invalid direct body child in {}: {}",
            page.display(),
            node_label(&child)
        )
        .into());
    }

    if main_count != 1 {
        return Err(format!(
            "expected exactly one direct main child in {}: found {main_count}",
            page.display()
        )
        .into());
    }
    if notes_count != 1 {
        return Err(format!(
            "expected exactly one direct notes section in {}: found {notes_count}",
            page.display()
        )
        .into());
    }

    Ok(())
}

fn validate_title_contract(page: &Path, document: &PageDocument, main: &NodeRef) -> Result<()> {
    let title = single_node(page, document, "title")?;
    let title_text = normalize_link_label(&title.text_contents());
    if title_text.is_empty() {
        return Err(format!("page title cannot be empty in {}", page.display()).into());
    }

    let children = meaningful_children(main);
    let Some(first_child) = children.first() else {
        return Err(format!("missing main heading in {}", page.display()).into());
    };
    if !is_element_named(first_child, "h1") {
        return Err(format!(
            "main heading must be the first meaningful child in {}",
            page.display()
        )
        .into());
    }

    let h1_count = select_nodes(document, "main h1").len();
    if h1_count != 1 {
        return Err(format!(
            "expected exactly one main h1 in {}: found {h1_count}",
            page.display()
        )
        .into());
    }

    let heading_text = normalize_link_label(&first_child.text_contents());
    if heading_text.is_empty() {
        return Err(format!("main heading cannot be empty in {}", page.display()).into());
    }
    if title_text != heading_text {
        return Err(format!(
            "title and main heading differ in {}: `{title_text}` != `{heading_text}`",
            page.display()
        )
        .into());
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
            return Err(
                format!("unsupported Fractal meta tag in {}: {name}", page.display()).into(),
            );
        }

        let Some(content) = attributes.get("content") else {
            return Err(format!(
                "missing content for required meta tag in {}: {name}",
                page.display()
            )
            .into());
        };
        found
            .entry(name.to_string())
            .or_default()
            .push(content.to_string());
    }

    for (name, expected_content) in allowed {
        let Some(values) = found.get(name) else {
            return Err(format!("missing required meta tag in {}: {name}", page.display()).into());
        };
        if values.len() != 1 {
            return Err(
                format!("duplicate required meta tag in {}: {name}", page.display()).into(),
            );
        }
        if name == "fractal:version" && values[0] != expected_content {
            return Err(format!(
                "unsupported page format version in {}: {} (expected {})",
                page.display(),
                values[0],
                expected_content
            )
            .into());
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
        return Err(format!(
            "missing exact Fractal stylesheet link in {}: expected {expected}",
            page.display()
        )
        .into());
    }

    Ok(())
}

fn validate_body_theme(page: &Path, theme: Theme, body: &NodeRef) -> Result<()> {
    let Some(element) = body.as_element() else {
        return Err(format!("body node is not an element in {}", page.display()).into());
    };
    let attributes = element.attributes.borrow();
    let expected = theme.as_str();
    if attributes.get("data-fractal-theme") != Some(expected) {
        return Err(format!(
            "body theme mismatch in {}: expected {expected}",
            page.display()
        )
        .into());
    }

    Ok(())
}

fn validate_notes_section(page: &Path, notes_section: &NodeRef) -> Result<()> {
    for child in meaningful_children(notes_section) {
        if !is_fractal_note_aside(&child) {
            return Err(format!(
                "invalid notes section child in {}: {}",
                page.display(),
                node_label(&child)
            )
            .into());
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
            return Err(format!(
                "note must be a direct child of the notes section in {}",
                page.display()
            )
            .into());
        }

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
            return Err(format!("unexpected h1 in {}", page.display()).into());
        }
        if name == "a" && !element.attributes.borrow().contains("data-fractal-link") {
            return Err(format!("manual link is not valid Fractal in {}", page.display()).into());
        }
        if !is_allowed_body_element(&name) {
            return Err(format!(
                "unsupported Fractal body element in {}: <{name}>",
                page.display()
            )
            .into());
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
        return Err(format!("missing first main h1 in {}", page.display()).into());
    }

    for child in children.into_iter().skip(start) {
        let Some(element) = child.as_element() else {
            return Err(format!(
                "unsupported direct content node in {}: {}",
                page.display(),
                node_label(&child)
            )
            .into());
        };
        let name = element.name.local.to_string();
        if !is_allowed_body_element(&name) {
            return Err(format!(
                "unsupported direct Fractal body element in {}: <{name}>",
                page.display()
            )
            .into());
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
                return Err(format!(
                    "list children must be li elements in {}: {}",
                    page.display(),
                    node_label(&child)
                )
                .into());
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
        let Some(scope) = attributes.get("data-fractal-link") else {
            return Err(format!("manual link is not valid Fractal in {}", page.display()).into());
        };
        let Some(href) = attributes.get("href") else {
            return Err(format!("generated link is missing href in {}", page.display()).into());
        };

        match scope {
            "page" => {
                let Some(target) = resolve_page_href(page_path, href) else {
                    return Err(format!(
                        "generated page link does not resolve in {}: {href}",
                        page.display()
                    )
                    .into());
                };
                if !known_page_paths.contains(&target) {
                    return Err(format!(
                        "generated page link target is missing in {}: {href}",
                        page.display()
                    )
                    .into());
                }
            }
            "note" => {
                let Some(note_id) = href.strip_prefix('#') else {
                    return Err(format!(
                        "generated note link must target a page-local note in {}: {href}",
                        page.display()
                    )
                    .into());
                };
                if !note_ids.contains(note_id) {
                    return Err(format!(
                        "generated note link target is missing in {}: {href}",
                        page.display()
                    )
                    .into());
                }
            }
            _ => {
                return Err(format!(
                    "unsupported generated link scope in {}: {scope}",
                    page.display()
                )
                .into())
            }
        }
    }

    Ok(())
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
        [] => Err(format!("missing {label} in {}", page.display()).into()),
        _ => Err(format!(
            "duplicate {label} in {}: {} found",
            page.display(),
            nodes.len()
        )
        .into()),
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

fn known_html_page_paths(page_paths: &[String]) -> BTreeSet<String> {
    page_paths
        .iter()
        .filter(|path| is_html_path(path))
        .cloned()
        .collect()
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
