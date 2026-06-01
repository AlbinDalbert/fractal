use crate::project::html::escape_html_attribute;
use crate::project::links::{inferred_link_scope, normalize_link_label, note_label_from_id};
use crate::project::types::{LinkEntry, NoteEntry};
use crate::Result;
use kuchiki::traits::*;
use kuchiki::NodeRef;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) struct PageDocument {
    pub(super) document: kuchiki::NodeRef,
}

impl PageDocument {
    pub(super) fn parse(html: &str) -> Self {
        Self {
            document: kuchiki::parse_html().one(html),
        }
    }

    pub(super) fn from_path(page: &Path) -> Result<Self> {
        Ok(Self::parse(&fs::read_to_string(page)?))
    }

    pub(super) fn has_notes_section(&self) -> bool {
        self.notes_section_count() > 0
    }

    pub(super) fn notes_section_count(&self) -> usize {
        self.notes_sections().len()
    }

    pub(super) fn notes_sections(&self) -> Vec<NodeRef> {
        self.document
            .select("section[data-fractal-notes]")
            .expect("static selector should parse")
            .map(|element| element.as_node().clone())
            .collect()
    }

    pub(super) fn single_notes_section(&self) -> Result<NodeRef> {
        let sections = self.notes_sections();
        match sections.as_slice() {
            [] => Err("missing notes section in page".into()),
            [section] => Ok(section.clone()),
            _ => Err(format!("multiple notes sections in page: {}", sections.len()).into()),
        }
    }

    pub(super) fn ensure_meta_tag(&self, name: &str, content: &str) -> Result<bool> {
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

    pub(super) fn ensure_stylesheet_link(&self, href: &str) -> Result<bool> {
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

    pub(super) fn ensure_body_theme(&self, theme: &str) -> Result<bool> {
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

    pub(super) fn ensure_single_notes_section(&self) -> Result<bool> {
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

    pub(super) fn note_node(&self, note_id: &str) -> Option<NodeRef> {
        self.document
            .select("section[data-fractal-notes] aside[data-fractal-note]")
            .expect("static selector should parse")
            .find_map(|element| {
                let attributes = element.attributes.borrow();
                (attributes.get("id") == Some(note_id)).then(|| element.as_node().clone())
            })
    }

    pub(super) fn to_html(&self) -> Result<String> {
        let mut serialized = Vec::new();
        self.document.serialize(&mut serialized)?;
        Ok(String::from_utf8(serialized)?)
    }

    pub(super) fn title(&self) -> Option<String> {
        self.element_text("title")
            .or_else(|| self.element_text("h1"))
            .map(|title| normalize_link_label(&title))
            .filter(|title| !title.is_empty())
    }

    pub(super) fn fractal_meta(&self) -> BTreeMap<String, String> {
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

    pub(super) fn notes(&self) -> Vec<NoteEntry> {
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

    pub(super) fn links(&self) -> Vec<LinkEntry> {
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
