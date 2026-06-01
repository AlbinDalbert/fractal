use crate::project::constants::{INDEX_PAGE, MANIFEST_FILE, PAGES_DIR, STYLE_FILE, WORKSPACE_DIR};
use crate::project::document::PageDocument;
use crate::project::html::{
    escape_html, insert_before_body_close, insert_before_head_close, insert_body_attribute,
};
use crate::project::notes::is_valid_note_id;
use crate::project::paths::{collect_page_paths, is_html_path, load_manifest};
use crate::project::render::{
    default_stylesheet, render_page_document, required_meta_tags, stylesheet_href,
};
use crate::project::types::Theme;
use crate::Result;
use kuchiki::NodeRef;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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

pub(super) fn validate_page_metadata(page: &Path) -> Result<()> {
    let document = PageDocument::from_path(page)?;

    match document.notes_section_count() {
        0 => return Err(format!("missing notes section in {}", page.display()).into()),
        1 => {}
        count => {
            return Err(format!(
                "duplicate notes section in {}: {count} sections",
                page.display()
            )
            .into())
        }
    }

    let meta = document.fractal_meta();

    for (name, content) in required_meta_tags() {
        if meta.get(name).map(|value| value.as_str()) != Some(content) {
            return Err(
                format!("missing required meta tag in {}: {}", page.display(), name).into(),
            );
        }
    }

    validate_note_ids(page, &document)?;

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

    match PageDocument::parse(&html).notes_section_count() {
        0 => {
            html = insert_before_body_close(
                &html,
                "    <section data-fractal-notes>\n    </section>\n",
            )?;
            changed = true;
        }
        1 => {}
        _ => {
            html = normalize_notes_sections(&html)?;
            changed = true;
        }
    }

    if changed {
        fs::write(page, html)?;
        println!("fixed {}", page.display());
    }

    Ok(())
}

fn validate_note_ids(page: &Path, document: &PageDocument) -> Result<()> {
    let mut seen = BTreeSet::new();

    for element in document
        .document
        .select("section[data-fractal-notes] aside[data-fractal-note]")
        .expect("static selector should parse")
    {
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

fn normalize_notes_sections(html: &str) -> Result<String> {
    let document = PageDocument::parse(html);
    let sections = document.notes_sections();

    if sections.is_empty() {
        let body = document
            .document
            .select_first("body")
            .map_err(|_| "missing body in page")?;
        body.as_node().append(NodeRef::new_text("\n    "));
        body.as_node().append(parse_notes_section()?);
        body.as_node().append(NodeRef::new_text("\n  "));
    } else {
        let primary = sections[0].clone();
        for duplicate in sections.iter().skip(1) {
            let children = duplicate.children().collect::<Vec<_>>();
            for child in children {
                primary.append(child);
            }
            duplicate.detach();
        }
    }

    document.to_html()
}

fn parse_notes_section() -> Result<NodeRef> {
    let document = PageDocument::parse("<section data-fractal-notes></section>");
    let section = document
        .document
        .select_first("section[data-fractal-notes]")
        .map_err(|_| "notes section markup must contain a data-fractal-notes section")?
        .as_node()
        .clone();
    section.detach();
    Ok(section)
}
