use crate::{FractalError, Result};
use brik::traits::*;
use brik::{NodeData, NodeRef};
use std::path::{Component, Path, PathBuf};

pub(crate) struct Document {
    root: NodeRef,
}

pub(crate) struct RawIframe {
    pub(crate) src: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) sandbox: Option<String>,
    pub(crate) has_srcdoc: bool,
}

pub(crate) struct UnlinkedTextNode {
    pub(crate) index: usize,
    pub(crate) text: String,
}

impl Document {
    pub(crate) fn parse(html: &str) -> Self {
        Self {
            root: brik::parse_html().one(html),
        }
    }

    pub(crate) fn title(&self) -> Option<String> {
        self.root
            .select_first("title")
            .ok()
            .or_else(|| self.root.select_first("h1").ok())
            .map(|node| normalize_space(&node.text_contents()))
            .filter(|title| !title.is_empty())
    }

    pub(crate) fn has_html_doctype(&self) -> bool {
        self.root
            .children()
            .any(|node| matches!(node.data(), NodeData::Doctype(value) if value.name.eq_ignore_ascii_case("html")))
    }

    pub(crate) fn has_native_marker(&self) -> bool {
        self.root
            .select("head meta[name]")
            .expect("static selector")
            .any(|node| {
                let attributes = node.attributes.borrow();
                attributes
                    .get("name")
                    .is_some_and(|value| value.eq_ignore_ascii_case("fractal-format"))
                    && attributes.get("content") == Some("1")
            })
    }

    pub(crate) fn native_root_count(&self) -> usize {
        self.root
            .select("main[data-fractal-document]")
            .expect("static selector")
            .count()
    }

    pub(crate) fn body_elements_outside_native_root(&self) -> Vec<String> {
        let Ok(body) = self.root.select_first("body") else {
            return Vec::new();
        };
        body.as_node()
            .children()
            .filter_map(|node| {
                let element = node.as_element()?;
                let is_native_root = element.name.local.as_ref() == "main"
                    && element
                        .attributes
                        .borrow()
                        .contains("data-fractal-document");
                (!is_native_root).then(|| element.name.local.to_string())
            })
            .collect()
    }

    pub(crate) fn unsupported_native_elements(&self) -> Vec<String> {
        const ALLOWED: &[&str] = &[
            "a",
            "abbr",
            "b",
            "blockquote",
            "br",
            "caption",
            "cite",
            "code",
            "col",
            "colgroup",
            "del",
            "em",
            "figcaption",
            "figure",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "hr",
            "i",
            "iframe",
            "img",
            "ins",
            "kbd",
            "li",
            "mark",
            "ol",
            "p",
            "pre",
            "q",
            "s",
            "samp",
            "small",
            "span",
            "strong",
            "sub",
            "sup",
            "table",
            "tbody",
            "td",
            "tfoot",
            "th",
            "thead",
            "time",
            "tr",
            "u",
            "ul",
            "var",
        ];
        let Ok(root) = self.root.select_first("main[data-fractal-document]") else {
            return Vec::new();
        };
        let mut unsupported: Vec<_> = root
            .as_node()
            .descendants()
            .filter_map(|node| {
                let name = node.as_element()?.name.local.to_string();
                (!ALLOWED.contains(&name.as_str())).then_some(name)
            })
            .collect();
        unsupported.sort();
        unsupported.dedup();
        unsupported
    }

    pub(crate) fn unsupported_native_head_elements(&self) -> Vec<String> {
        const ALLOWED: &[&str] = &["link", "meta", "style", "title"];
        let Ok(head) = self.root.select_first("head") else {
            return Vec::new();
        };
        let mut unsupported: Vec<_> = head
            .as_node()
            .descendants()
            .filter_map(|node| {
                let name = node.as_element()?.name.local.to_string();
                (!ALLOWED.contains(&name.as_str())).then_some(name)
            })
            .collect();
        unsupported.sort();
        unsupported.dedup();
        unsupported
    }

    pub(crate) fn text(&self) -> String {
        let node = self
            .root
            .select_first("body")
            .ok()
            .map(|node| node.as_node().clone())
            .unwrap_or_else(|| self.root.clone());
        normalize_space(&node.text_contents())
    }

    pub(crate) fn unlinked_text_nodes(&self) -> Vec<UnlinkedTextNode> {
        let root = self
            .root
            .select_first("main[data-fractal-document]")
            .or_else(|_| self.root.select_first("body"))
            .map(|node| node.as_node().clone())
            .unwrap_or_else(|_| self.root.clone());
        let mut nodes = Vec::new();
        let mut index = 0;
        for node in root.descendants() {
            let NodeData::Text(text) = node.data() else {
                continue;
            };
            let excluded = node.ancestors().any(|ancestor| {
                ancestor.as_element().is_some_and(|element| {
                    matches!(
                        element.name.local.as_ref(),
                        "a" | "script" | "style" | "code" | "pre"
                    )
                })
            });
            if !excluded {
                nodes.push(UnlinkedTextNode {
                    index,
                    text: text.borrow().to_string(),
                });
            }
            index += 1;
        }
        nodes
    }

    pub(crate) fn raw_links(&self) -> Vec<(String, String)> {
        self.root
            .select("a[href]")
            .expect("static selector")
            .filter_map(|link| {
                let href = link.attributes.borrow().get("href")?.to_string();
                Some((href, normalize_space(&link.text_contents())))
            })
            .collect()
    }

    pub(crate) fn raw_iframes(&self) -> Vec<RawIframe> {
        self.root
            .select("iframe")
            .expect("static selector")
            .map(|iframe| {
                let attributes = iframe.attributes.borrow();
                RawIframe {
                    src: attributes.get("src").map(str::to_string),
                    title: attributes.get("title").map(str::to_string),
                    sandbox: attributes.get("sandbox").map(str::to_string),
                    has_srcdoc: attributes.contains("srcdoc"),
                }
            })
            .collect()
    }

    pub(crate) fn rewrite_internal_target(
        &self,
        source: &str,
        old_target: &str,
        new_target: &str,
    ) -> usize {
        let mut count = 0;
        for link in self.root.select("a[href]").expect("static selector") {
            let mut attributes = link.attributes.borrow_mut();
            let Some(href) = attributes.get("href").map(str::to_string) else {
                continue;
            };
            if resolve_internal_href(source, &href).as_deref() != Some(old_target) {
                continue;
            }
            let suffix = href
                .find(['?', '#'])
                .map(|index| &href[index..])
                .unwrap_or("");
            let relative = relative_href(source, new_target);
            attributes.insert("href", format!("{relative}{suffix}"));
            count += 1;
        }
        for iframe in self.root.select("iframe[src]").expect("static selector") {
            let mut attributes = iframe.attributes.borrow_mut();
            if attributes.contains("srcdoc") {
                continue;
            }
            let Some(src) = attributes.get("src").map(str::to_string) else {
                continue;
            };
            if resolve_internal_href(source, &src).as_deref() != Some(old_target) {
                continue;
            }
            let suffix = src
                .find(['?', '#'])
                .map(|index| &src[index..])
                .unwrap_or("");
            let relative = relative_href(source, new_target);
            attributes.insert("src", format!("{relative}{suffix}"));
            count += 1;
        }
        count
    }

    pub(crate) fn rewrite_source_location(&self, old_source: &str, new_source: &str) -> usize {
        let mut count = 0;
        for link in self.root.select("a[href]").expect("static selector") {
            let mut attributes = link.attributes.borrow_mut();
            let Some(href) = attributes.get("href").map(str::to_string) else {
                continue;
            };
            let Some(mut target) = resolve_internal_href(old_source, &href) else {
                continue;
            };
            if target == old_source {
                target = new_source.to_string();
            }
            let suffix = href
                .find(['?', '#'])
                .map(|index| &href[index..])
                .unwrap_or("");
            attributes.insert(
                "href",
                format!("{}{}", relative_href(new_source, &target), suffix),
            );
            count += 1;
        }
        for iframe in self.root.select("iframe[src]").expect("static selector") {
            let mut attributes = iframe.attributes.borrow_mut();
            if attributes.contains("srcdoc") {
                continue;
            }
            let Some(src) = attributes.get("src").map(str::to_string) else {
                continue;
            };
            let Some(mut target) = resolve_internal_href(old_source, &src) else {
                continue;
            };
            if target == old_source {
                target = new_source.to_string();
            }
            let suffix = src
                .find(['?', '#'])
                .map(|index| &src[index..])
                .unwrap_or("");
            attributes.insert(
                "src",
                format!("{}{}", relative_href(new_source, &target), suffix),
            );
            count += 1;
        }
        count
    }

    pub(crate) fn insert_link(&self, text: &str, href: &str) -> Result<bool> {
        if text.trim().is_empty() {
            return Err(FractalError::invalid_input("link text cannot be empty"));
        }

        for node in self.root.descendants() {
            let NodeData::Text(contents) = node.data() else {
                continue;
            };
            if node.ancestors().any(|ancestor| {
                ancestor.as_element().is_some_and(|element| {
                    matches!(
                        element.name.local.as_ref(),
                        "a" | "title" | "script" | "style" | "code" | "pre"
                    )
                })
            }) {
                continue;
            }

            let original = contents.borrow().to_string();
            let Some(start) = find_case_insensitive(&original, text) else {
                continue;
            };
            let end = start + text.len();
            if !original.is_char_boundary(start) || !original.is_char_boundary(end) {
                continue;
            }

            let before = &original[..start];
            let matched = &original[start..end];
            let after = &original[end..];
            if !before.is_empty() {
                node.insert_before(NodeRef::new_text(before));
            }
            node.insert_before(parse_link(href, matched)?);
            if !after.is_empty() {
                node.insert_before(NodeRef::new_text(after));
            }
            node.detach();
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn serialize(&self) -> Result<String> {
        let mut bytes = Vec::new();
        self.root.serialize(&mut bytes)?;
        Ok(String::from_utf8(bytes)?)
    }

    pub(crate) fn flatten_for_html(&self, source: &str) -> Result<()> {
        let links: Vec<_> = self
            .root
            .select("a[href]")
            .expect("static selector")
            .collect();
        for link in links {
            let href = link.attributes.borrow().get("href").map(str::to_string);
            let Some(href) = href else {
                continue;
            };
            if href.starts_with('#') || is_external_href(&href) {
                continue;
            }
            if resolve_internal_href(source, &href).is_some() {
                unwrap_element(link.as_node().clone());
            }
        }

        let media: Vec<_> = self
            .root
            .select("img, iframe")
            .expect("static selector")
            .filter(|node| {
                !node.as_node().ancestors().any(|ancestor| {
                    ancestor.as_element().is_some_and(|element| {
                        matches!(element.name.local.as_ref(), "img" | "iframe")
                    })
                })
            })
            .collect();
        for media in media {
            let marker = match media.name.local.as_ref() {
                "img" => " [image] ",
                "iframe" => " [iframe] ",
                _ => continue,
            };
            media.as_node().insert_before(NodeRef::new_text(marker));
            media.as_node().detach();
        }

        let links: Vec<_> = self
            .root
            .select("link")
            .expect("static selector")
            .filter(|node| {
                node.attributes.borrow().get("rel").is_some_and(|rel| {
                    rel.split_ascii_whitespace()
                        .any(|value| value.eq_ignore_ascii_case("stylesheet"))
                })
            })
            .collect();
        for link in links {
            link.as_node().detach();
        }

        let markers: Vec<_> = self
            .root
            .select("meta[name]")
            .expect("static selector")
            .filter(|node| {
                node.attributes
                    .borrow()
                    .get("name")
                    .is_some_and(|name| name.eq_ignore_ascii_case("fractal-format"))
            })
            .collect();
        for marker in markers {
            marker.as_node().detach();
        }

        if let Ok(root) = self.root.select_first("main[data-fractal-document]") {
            root.attributes.borrow_mut().remove("data-fractal-document");
        }
        Ok(())
    }

    pub(crate) fn export_text(&self) -> String {
        let root = self
            .root
            .select_first("body")
            .map(|node| node.as_node().clone())
            .unwrap_or_else(|_| self.root.clone());
        let mut text = String::new();
        append_export_text(&root, &mut text);
        normalize_space(&text)
    }

    pub(crate) fn append_to_body(&self, html: &str) -> Result<()> {
        let body = self
            .root
            .select_first("body")
            .map_err(|_| FractalError::invalid_input("native document needs a body"))?;
        let fragment = brik::parse_html().one(format!("<body>{html}</body>"));
        let fragment_body = fragment
            .select_first("body")
            .map_err(|_| FractalError::invalid_input("could not create export content"))?;
        let children: Vec<_> = fragment_body.as_node().children().collect();
        for child in children {
            body.as_node().append(child);
        }
        Ok(())
    }
}

fn unwrap_element(node: NodeRef) {
    let children: Vec<_> = node.children().collect();
    for child in children {
        node.insert_before(child);
    }
    node.detach();
}

fn append_export_text(node: &NodeRef, output: &mut String) {
    match node.data() {
        NodeData::Text(text) => output.push_str(&text.borrow()),
        NodeData::Element(element) => {
            let name = element.name.local.as_ref();
            match name {
                "img" => output.push_str(" [image] "),
                "iframe" => output.push_str(" [iframe] "),
                "script" | "style" => {}
                _ => {
                    let block = matches!(
                        name,
                        "address"
                            | "article"
                            | "blockquote"
                            | "dd"
                            | "div"
                            | "dl"
                            | "dt"
                            | "figcaption"
                            | "figure"
                            | "footer"
                            | "h1"
                            | "h2"
                            | "h3"
                            | "h4"
                            | "h5"
                            | "h6"
                            | "header"
                            | "li"
                            | "main"
                            | "nav"
                            | "ol"
                            | "p"
                            | "pre"
                            | "section"
                            | "table"
                            | "td"
                            | "th"
                            | "tr"
                            | "ul"
                    );
                    if block {
                        output.push(' ');
                    }
                    for child in node.children() {
                        append_export_text(&child, output);
                    }
                    if block {
                        output.push(' ');
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_link(href: &str, text: &str) -> Result<NodeRef> {
    let html = format!(
        "<a href=\"{}\">{}</a>",
        escape_attribute(href),
        escape_html(text)
    );
    let root = brik::parse_html().one(html);
    let node = root
        .select_first("a")
        .map_err(|_| FractalError::invalid_input("could not create link"))?
        .as_node()
        .clone();
    node.detach();
    Ok(node)
}

pub(crate) fn is_external_href(href: &str) -> bool {
    href.starts_with("//")
        || href.split_once(':').is_some_and(|(scheme, _)| {
            scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
}

pub(crate) fn resolve_internal_href(source: &str, href: &str) -> Option<String> {
    if href.is_empty() || href.starts_with('#') || is_external_href(href) {
        return None;
    }
    let path = href.split(['?', '#']).next()?;
    if path.is_empty() {
        return None;
    }
    let mut resolved = if path.starts_with('/') {
        PathBuf::new()
    } else {
        Path::new(source)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf()
    };
    for component in Path::new(path.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(resolved.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn relative_href(source: &str, target: &str) -> String {
    let source_dir = Path::new(source).parent().unwrap_or_else(|| Path::new(""));
    let source_parts: Vec<_> = source_dir.components().collect();
    let target_parts: Vec<_> = Path::new(target).components().collect();
    let shared = source_parts
        .iter()
        .zip(&target_parts)
        .take_while(|(left, right)| left == right)
        .count();
    let mut output = PathBuf::new();
    for _ in shared..source_parts.len() {
        output.push("..");
    }
    for part in &target_parts[shared..] {
        output.push(part.as_os_str());
    }
    output.to_string_lossy().replace('\\', "/")
}

pub(crate) fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|start| {
        let end = start + needle.len();
        haystack.is_char_boundary(*start)
            && haystack.is_char_boundary(end)
            && haystack[*start..end].eq_ignore_ascii_case(needle)
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn escape_attribute(value: &str) -> String {
    escape_html(value).replace('"', "&quot;")
}
