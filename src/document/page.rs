use crate::document::html::{escape_html, escape_html_attribute};
use crate::graph::links::{
    inferred_link_scope, normalize_link_label, note_label_from_id, relative_href, resolve_page_href,
};
use crate::types::{LinkEntry, NoteEntry};
use crate::Result;
use kuchiki::traits::*;
use kuchiki::NodeRef;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(crate) struct PageDocument {
    pub(crate) document: kuchiki::NodeRef,
}

impl PageDocument {
    pub(crate) fn parse(html: &str) -> Self {
        Self {
            document: kuchiki::parse_html().one(html),
        }
    }

    pub(crate) fn from_path(page: &Path) -> Result<Self> {
        Ok(Self::parse(&fs::read_to_string(page)?))
    }

    pub(crate) fn has_notes_section(&self) -> bool {
        self.notes_section_count() > 0
    }

    pub(crate) fn notes_section_count(&self) -> usize {
        self.notes_sections().len()
    }

    pub(crate) fn notes_sections(&self) -> Vec<NodeRef> {
        self.document
            .select("section[data-fractal-notes]")
            .expect("static selector should parse")
            .map(|element| element.as_node().clone())
            .collect()
    }

    pub(crate) fn single_notes_section(&self) -> Result<NodeRef> {
        let sections = self.notes_sections();
        match sections.as_slice() {
            [] => Err("missing notes section in page".into()),
            [section] => Ok(section.clone()),
            _ => Err(format!("multiple notes sections in page: {}", sections.len()).into()),
        }
    }

    pub(crate) fn ensure_meta_tag(&self, name: &str, content: &str) -> Result<bool> {
        if self.fractal_meta().contains_key(name) {
            return Ok(false);
        }

        let head = self.head_node()?;
        head.append(NodeRef::new_text("\n    "));
        head.append(parse_document_node(
            &format!(
                "<meta name=\"{}\" content=\"{}\">",
                escape_html_attribute(name),
                escape_html_attribute(content)
            ),
            "meta[name][content]",
        )?);
        head.append(NodeRef::new_text("\n  "));
        Ok(true)
    }

    pub(crate) fn set_meta_tag(&self, name: &str, content: &str) -> Result<bool> {
        let mut updated = false;

        for element in self
            .document
            .select("meta[name][content]")
            .expect("static selector should parse")
        {
            let mut attributes = element.attributes.borrow_mut();
            if attributes.get("name") != Some(name) {
                continue;
            }

            if attributes.get("content") == Some(content) {
                return Ok(false);
            }

            attributes.insert("content", content.to_string());
            updated = true;
        }

        if updated {
            return Ok(true);
        }

        self.ensure_meta_tag(name, content)
    }

    pub(crate) fn ensure_stylesheet_link(&self, href: &str) -> Result<bool> {
        if self
            .document
            .select("link[href][rel]")
            .expect("static selector should parse")
            .any(|element| {
                let attributes = element.attributes.borrow();
                let rel = attributes.get("rel").unwrap_or_default();
                let href = attributes.get("href").unwrap_or_default();
                rel.split_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                    && href.contains(".fractal/style.css")
            })
        {
            return Ok(false);
        }

        let head = self.head_node()?;
        head.append(NodeRef::new_text("\n    "));
        head.append(parse_document_node(
            &format!(
                "<link rel=\"stylesheet\" href=\"{}\">",
                escape_html_attribute(href)
            ),
            "link[rel][href]",
        )?);
        head.append(NodeRef::new_text("\n  "));
        Ok(true)
    }

    pub(crate) fn set_stylesheet_href(&self, href: &str) -> Result<bool> {
        let mut changed = false;
        let mut found = false;

        for element in self
            .document
            .select("link[href][rel]")
            .expect("static selector should parse")
        {
            let mut attributes = element.attributes.borrow_mut();
            let rel = attributes.get("rel").unwrap_or_default();
            let existing_href = attributes.get("href").unwrap_or_default();
            let is_fractal_stylesheet = rel
                .split_whitespace()
                .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                && existing_href.contains(".fractal/style.css");

            if !is_fractal_stylesheet {
                continue;
            }

            found = true;
            if existing_href != href {
                attributes.insert("href", href.to_string());
                changed = true;
            }
        }

        if found {
            Ok(changed)
        } else {
            self.ensure_stylesheet_link(href)
        }
    }

    pub(crate) fn ensure_body_theme(&self, theme: &str) -> Result<bool> {
        let body = self.body_node()?;
        let Some(element) = body.as_element() else {
            return Err("body node is not an element".into());
        };
        let mut attributes = element.attributes.borrow_mut();
        if attributes.contains("data-fractal-theme") {
            return Ok(false);
        }

        attributes.insert("data-fractal-theme", theme.to_string());
        Ok(true)
    }

    pub(crate) fn ensure_single_notes_section(&self) -> Result<bool> {
        let sections = self.notes_sections();
        match sections.as_slice() {
            [] => {
                let body = self.body_node()?;
                body.append(NodeRef::new_text("\n    "));
                body.append(parse_document_node(
                    "<section data-fractal-notes></section>",
                    "section[data-fractal-notes]",
                )?);
                body.append(NodeRef::new_text("\n  "));
                Ok(true)
            }
            [_] => Ok(false),
            [primary, duplicates @ ..] => {
                for duplicate in duplicates {
                    let children = duplicate.children().collect::<Vec<_>>();
                    for child in children {
                        primary.append(child);
                    }
                    duplicate.detach();
                }
                Ok(true)
            }
        }
    }

    pub(crate) fn note_node(&self, note_id: &str) -> Option<NodeRef> {
        self.document
            .select("section[data-fractal-notes] aside[data-fractal-note]")
            .expect("static selector should parse")
            .find_map(|element| {
                let attributes = element.attributes.borrow();
                (attributes.get("id") == Some(note_id)).then(|| element.as_node().clone())
            })
    }

    pub(crate) fn to_html(&self) -> Result<String> {
        let mut serialized = Vec::new();
        self.document.serialize(&mut serialized)?;
        Ok(String::from_utf8(serialized)?)
    }

    pub(crate) fn main_body_html(&self) -> Result<String> {
        let main = self.main_node()?;
        let children = main.children().collect::<Vec<_>>();
        let body_start = children
            .iter()
            .position(|child| is_element_named(child, "h1"))
            .map(|index| index + 1)
            .unwrap_or(0);
        let mut serialized = Vec::new();

        for child in children.into_iter().skip(body_start) {
            child.serialize(&mut serialized)?;
        }

        Ok(String::from_utf8(serialized)?)
    }

    pub(crate) fn set_title(&self, title: &str) -> Result<bool> {
        let before = self.to_html()?;

        match self.document.select_first("title").ok() {
            Some(title_element) => replace_children_with_text(title_element.as_node(), title),
            None => {
                let head = self.head_node()?;
                head.append(NodeRef::new_text("\n    "));
                head.append(parse_document_node(
                    &format!("<title>{}</title>", escape_html(title)),
                    "title",
                )?);
                head.append(NodeRef::new_text("\n  "));
            }
        }

        match self.document.select_first("main h1").ok() {
            Some(heading) => replace_children_with_text(heading.as_node(), title),
            None => {
                let main = self.main_node()?;
                prepend_child(
                    &main,
                    parse_document_node(&format!("<h1>{}</h1>", escape_html(title)), "h1")?,
                );
            }
        }

        Ok(before != self.to_html()?)
    }

    pub(crate) fn set_main_body_html(&self, body_html: &str) -> Result<bool> {
        let before = self.to_html()?;
        let main = self.main_node()?;
        let heading = direct_child_element(&main, "h1")
            .or_else(|| {
                self.document
                    .select_first("main h1")
                    .ok()
                    .map(|h| h.as_node().clone())
            })
            .unwrap_or_else(|| {
                let title = self.title().unwrap_or_else(|| "Untitled".to_string());
                parse_document_node(&format!("<h1>{}</h1>", escape_html(&title)), "h1")
                    .expect("generated heading should parse")
            });
        let body_nodes = parse_main_fragment_children(body_html)?;

        for child in main.children().collect::<Vec<_>>() {
            child.detach();
        }

        main.append(NodeRef::new_text("\n      "));
        main.append(heading);
        for child in body_nodes {
            main.append(NodeRef::new_text("\n      "));
            main.append(child);
        }
        main.append(NodeRef::new_text("\n    "));

        Ok(before != self.to_html()?)
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.element_text("title")
            .or_else(|| self.element_text("h1"))
            .map(|title| normalize_link_label(&title))
            .filter(|title| !title.is_empty())
    }

    pub(crate) fn fractal_meta(&self) -> BTreeMap<String, String> {
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

    pub(crate) fn notes(&self) -> Vec<NoteEntry> {
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

    pub(crate) fn links(&self) -> Vec<LinkEntry> {
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

    pub(crate) fn rewrite_page_hrefs(
        &self,
        from_page: &str,
        old_target: &str,
        new_href: &str,
    ) -> usize {
        let mut updated = 0;

        for element in self
            .document
            .select("a[href]")
            .expect("static selector should parse")
        {
            let mut attributes = element.attributes.borrow_mut();
            let Some(href) = attributes.get("href") else {
                continue;
            };

            if resolve_page_href(from_page, href).as_deref() != Some(old_target) {
                continue;
            }

            let rewritten = rewrite_href_path(href, new_href);
            if rewritten == href {
                continue;
            }

            attributes.insert("href", rewritten);
            updated += 1;
        }

        updated
    }

    pub(crate) fn rewrite_relative_page_hrefs_for_move(
        &self,
        old_page: &str,
        new_page: &str,
    ) -> usize {
        let mut updated = 0;

        for element in self
            .document
            .select("a[href]")
            .expect("static selector should parse")
        {
            let mut attributes = element.attributes.borrow_mut();
            let Some(href) = attributes.get("href") else {
                continue;
            };
            if href.starts_with('#') {
                continue;
            }

            let Some(mut target) = resolve_page_href(old_page, href) else {
                continue;
            };
            if target == old_page {
                target = new_page.to_string();
            }

            let rewritten = rewrite_href_path(href, &relative_href(new_page, &target));
            if rewritten == href {
                continue;
            }

            attributes.insert("href", rewritten);
            updated += 1;
        }

        updated
    }

    fn element_text(&self, selector: &str) -> Option<String> {
        self.document
            .select_first(selector)
            .ok()
            .map(|element| element.text_contents())
    }

    fn head_node(&self) -> Result<NodeRef> {
        Ok(self
            .document
            .select_first("head")
            .map_err(|_| "missing head in page")?
            .as_node()
            .clone())
    }

    fn body_node(&self) -> Result<NodeRef> {
        Ok(self
            .document
            .select_first("body")
            .map_err(|_| "missing body in page")?
            .as_node()
            .clone())
    }

    fn main_node(&self) -> Result<NodeRef> {
        Ok(self
            .document
            .select_first("main")
            .map_err(|_| "missing main section in page")?
            .as_node()
            .clone())
    }
}

fn parse_document_node(html: &str, selector: &str) -> Result<NodeRef> {
    let document = PageDocument::parse(html);
    let node = document
        .document
        .select_first(selector)
        .map_err(|_| format!("markup must contain `{selector}`"))?
        .as_node()
        .clone();
    node.detach();
    Ok(node)
}

fn parse_main_fragment_children(html: &str) -> Result<Vec<NodeRef>> {
    let document = PageDocument::parse(&format!("<main>{html}</main>"));
    let main = document.main_node()?;
    let children = main.children().collect::<Vec<_>>();
    for child in &children {
        child.detach();
    }
    Ok(children)
}

fn is_element_named(node: &NodeRef, name: &str) -> bool {
    node.as_element()
        .map(|element| element.name.local.to_string() == name)
        .unwrap_or(false)
}

fn direct_child_element(parent: &NodeRef, name: &str) -> Option<NodeRef> {
    parent
        .children()
        .find(|child| is_element_named(child, name))
}

fn replace_children_with_text(node: &NodeRef, text: &str) {
    for child in node.children().collect::<Vec<_>>() {
        child.detach();
    }
    node.append(NodeRef::new_text(text));
}

fn prepend_child(parent: &NodeRef, child: NodeRef) {
    if let Some(first_child) = parent.first_child() {
        first_child.insert_before(child);
    } else {
        parent.append(child);
    }
}

fn rewrite_href_path(href: &str, path: &str) -> String {
    let suffix_start = href
        .char_indices()
        .find_map(|(index, character)| matches!(character, '?' | '#').then_some(index))
        .unwrap_or(href.len());
    format!("{}{}", path, &href[suffix_start..])
}
