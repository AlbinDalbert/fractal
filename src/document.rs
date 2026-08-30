use crate::types::DerivedLink;
use crate::{FractalError, Result};
use brik::traits::*;
use brik::{NodeData, NodeRef};
use std::collections::{BTreeMap, BTreeSet};
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

    pub(crate) fn set_title(&self, title: &str) {
        for selector in [
            "title",
            "main[data-fractal-document] h1[data-fractal-title]",
        ] {
            if let Ok(node) = self.root.select_first(selector) {
                let node = node.as_node();
                for child in node.children().collect::<Vec<_>>() {
                    child.detach();
                }
                node.append(NodeRef::new_text(title));
            }
        }
    }

    pub(crate) fn managed_title_count(&self) -> usize {
        self.root
            .select("main[data-fractal-document] > h1[data-fractal-title]")
            .expect("static selector")
            .count()
    }

    pub(crate) fn managed_style_count(&self) -> usize {
        self.root
            .select("head > style[data-fractal-style]")
            .expect("static selector")
            .count()
    }

    pub(crate) fn content_html(&self) -> Result<String> {
        let main = self.native_main()?;
        serialize_children_matching(main.as_node(), |child| !is_managed_title(child))
    }

    pub(crate) fn set_content_html(&self, html: &str) -> Result<()> {
        let fragment = body_fragment(html)?;
        if fragment
            .select("[data-fractal-title]")
            .expect("static selector")
            .next()
            .is_some()
        {
            return Err(FractalError::invalid_input(
                "page content cannot contain a Fractal-owned title",
            ));
        }
        let main = self.native_main()?;
        for child in main.as_node().children().collect::<Vec<_>>() {
            if !is_managed_title(&child) {
                child.detach();
            }
        }
        let body = fragment.select_first("body").expect("fragment has a body");
        for child in body.as_node().children().collect::<Vec<_>>() {
            main.as_node().append(child);
        }
        Ok(())
    }

    pub(crate) fn managed_style_css(&self) -> Result<String> {
        let mut styles = self
            .root
            .select("head > style[data-fractal-style]")
            .expect("static selector");
        let style = styles.next().ok_or_else(|| {
            FractalError::invalid_input("native document needs a managed style; repair it first")
        })?;
        if styles.next().is_some() {
            return Err(FractalError::invalid_input(
                "native document has more than one managed style",
            ));
        }
        Ok(style.text_contents())
    }

    pub(crate) fn set_managed_style_css(&self, css: &str) -> Result<()> {
        let style = self
            .root
            .select_first("head > style[data-fractal-style]")
            .map_err(|_| FractalError::invalid_input("native document needs a managed style"))?;
        for child in style.as_node().children().collect::<Vec<_>>() {
            child.detach();
        }
        style.as_node().append(NodeRef::new_text(css));
        Ok(())
    }

    pub(crate) fn user_metadata_html(&self) -> Result<String> {
        let head = self.head()?;
        serialize_children_matching(head.as_node(), is_user_meta)
    }

    pub(crate) fn head_links_html(&self) -> Result<String> {
        let head = self.head()?;
        serialize_children_matching(head.as_node(), |child| element_name(child) == Some("link"))
    }

    pub(crate) fn set_user_metadata_html(&self, html: &str) -> Result<()> {
        let nodes = head_fragment_nodes(html, "meta")?;
        if nodes.iter().any(|node| !is_user_meta(node)) {
            return Err(FractalError::invalid_input(
                "required Fractal metadata cannot be changed through the metadata section",
            ));
        }
        self.replace_head_children(is_user_meta, nodes)
    }

    pub(crate) fn set_head_links_html(&self, html: &str) -> Result<()> {
        let nodes = head_fragment_nodes(html, "link")?;
        self.replace_head_children(|node| element_name(node) == Some("link"), nodes)
    }

    pub(crate) fn repair_managed_structure(&self, default_style: &str) -> Result<()> {
        let main = self.native_main()?;
        if self.managed_title_count() == 0 {
            let title = self.title().unwrap_or_else(|| "Untitled".into());
            if let Ok(h1) = self.root.select_first("main[data-fractal-document] > h1") {
                h1.attributes
                    .borrow_mut()
                    .insert("data-fractal-title", String::new());
            } else {
                let fragment = body_fragment(&format!(
                    "<h1 data-fractal-title>{}</h1>",
                    escape_html(&title)
                ))?;
                let h1 = fragment.select_first("h1").expect("created title");
                if let Some(first) = main.as_node().first_child() {
                    first.insert_before(h1.as_node().clone());
                } else {
                    main.as_node().append(h1.as_node().clone());
                }
            }
        }
        if self.managed_style_count() == 0 {
            if let Ok(style) = self.root.select_first("head > style") {
                style
                    .attributes
                    .borrow_mut()
                    .insert("data-fractal-style", String::new());
            } else {
                let nodes = head_fragment_nodes(
                    &format!("<style data-fractal-style>{default_style}</style>"),
                    "style",
                )?;
                let head = self.head()?;
                for node in nodes {
                    head.as_node().append(node);
                }
            }
        }
        Ok(())
    }

    fn native_main(&self) -> Result<brik::NodeDataRef<brik::ElementData>> {
        self.root
            .select_first("main[data-fractal-document]")
            .map_err(|_| FractalError::invalid_input("native document needs a document root"))
    }

    fn head(&self) -> Result<brik::NodeDataRef<brik::ElementData>> {
        self.root
            .select_first("head")
            .map_err(|_| FractalError::invalid_input("native document needs a head"))
    }

    fn replace_head_children<F>(&self, matches: F, nodes: Vec<NodeRef>) -> Result<()>
    where
        F: Fn(&NodeRef) -> bool,
    {
        let head = self.head()?;
        for child in head.as_node().children().collect::<Vec<_>>() {
            if matches(&child) {
                child.detach();
            }
        }
        for node in nodes {
            head.as_node().append(node);
        }
        Ok(())
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

    pub(crate) fn rewrite_internal_prefix(
        &self,
        source: &str,
        new_source: &str,
        old_prefix: &str,
        new_prefix: &str,
    ) -> usize {
        let mut count = 0;
        for selector in ["a[href]", "iframe[src]"] {
            for node in self.root.select(selector).expect("static selector") {
                let attribute = if selector.starts_with('a') {
                    "href"
                } else {
                    "src"
                };
                let mut attributes = node.attributes.borrow_mut();
                if attribute == "src" && attributes.contains("srcdoc") {
                    continue;
                }
                let Some(value) = attributes.get(attribute).map(str::to_string) else {
                    continue;
                };
                let Some(target) = resolve_internal_href(source, &value) else {
                    continue;
                };
                let old = Path::new(old_prefix);
                if !Path::new(&target).starts_with(old) {
                    continue;
                }
                let suffix = value
                    .find(['?', '#'])
                    .map(|index| &value[index..])
                    .unwrap_or("");
                let remainder = Path::new(&target)
                    .strip_prefix(old)
                    .expect("prefix checked");
                let target = Path::new(new_prefix)
                    .join(remainder)
                    .to_string_lossy()
                    .replace('\\', "/");
                attributes.insert(
                    attribute,
                    format!("{}{suffix}", relative_href(new_source, &target)),
                );
                count += 1;
            }
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

    pub(crate) fn flatten_for_html(
        &self,
        source: &str,
        native_targets: &BTreeSet<String>,
        derived_links: &[DerivedLink],
    ) -> Result<()> {
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
            let Some(target) = resolve_internal_href(source, &href) else {
                continue;
            };
            if native_targets.contains(&target) {
                link.attributes
                    .borrow_mut()
                    .insert("href", export_reference_href(&target));
            } else {
                unwrap_element(link.as_node().clone());
            }
        }

        insert_derived_export_links(self, derived_links, &BTreeMap::new())?;

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

    pub(crate) fn folder_export_content(
        &self,
        source: &str,
        included_targets: &BTreeMap<String, String>,
        reference_targets: &BTreeSet<String>,
        derived_links: &[DerivedLink],
    ) -> Result<String> {
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
            let Some(target) = resolve_internal_href(source, &href) else {
                continue;
            };
            if let Some(id) = included_targets.get(&target) {
                link.attributes
                    .borrow_mut()
                    .insert("href", format!("#{id}"));
            } else if reference_targets.contains(&target) {
                link.attributes
                    .borrow_mut()
                    .insert("href", export_reference_href(&target));
            } else {
                unwrap_element(link.as_node().clone());
            }
        }
        insert_derived_export_links(self, derived_links, included_targets)?;

        let media: Vec<_> = self
            .root
            .select("img, iframe")
            .expect("static selector")
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

        let main = self
            .root
            .select_first("main[data-fractal-document]")
            .map_err(|_| FractalError::invalid_input("native document needs a document root"))?;
        let mut skipped_title = false;
        let mut output = Vec::new();
        for child in main.as_node().children() {
            if !skipped_title
                && child
                    .as_element()
                    .is_some_and(|element| element.name.local.as_ref() == "h1")
            {
                skipped_title = true;
                continue;
            }
            child.serialize(&mut output)?;
        }
        Ok(String::from_utf8(output)?)
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

    pub(crate) fn append_to_main(&self, html: &str) -> Result<()> {
        let main = self
            .root
            .select_first("main")
            .map_err(|_| FractalError::invalid_input("native document needs a main element"))?;
        let fragment = brik::parse_html().one(format!("<body>{html}</body>"));
        let fragment_body = fragment
            .select_first("body")
            .map_err(|_| FractalError::invalid_input("could not create export content"))?;
        let children: Vec<_> = fragment_body.as_node().children().collect();
        for child in children {
            main.as_node().append(child);
        }
        Ok(())
    }
}

fn element_name(node: &NodeRef) -> Option<&str> {
    Some(node.as_element()?.name.local.as_ref())
}

fn is_managed_title(node: &NodeRef) -> bool {
    node.as_element().is_some_and(|element| {
        element.name.local.as_ref() == "h1"
            && element.attributes.borrow().contains("data-fractal-title")
    })
}

fn is_user_meta(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    if element.name.local.as_ref() != "meta" {
        return false;
    }
    let attributes = element.attributes.borrow();
    if attributes.contains("charset") {
        return false;
    }
    !attributes.get("name").is_some_and(|name| {
        name.eq_ignore_ascii_case("fractal-format") || name.eq_ignore_ascii_case("viewport")
    })
}

fn serialize_children_matching<F>(parent: &NodeRef, matches: F) -> Result<String>
where
    F: Fn(&NodeRef) -> bool,
{
    let mut output = Vec::new();
    for child in parent.children() {
        if matches(&child) {
            child.serialize(&mut output)?;
        }
    }
    Ok(String::from_utf8(output)?)
}

fn body_fragment(html: &str) -> Result<NodeRef> {
    let fragment = brik::parse_html().one(format!("<body>{html}</body>"));
    fragment
        .select_first("body")
        .map_err(|_| FractalError::invalid_input("could not parse page content"))?;
    Ok(fragment)
}

fn head_fragment_nodes(html: &str, expected: &str) -> Result<Vec<NodeRef>> {
    let fragment = brik::parse_html().one(format!("<html><head>{html}</head><body></body></html>"));
    let head = fragment
        .select_first("head")
        .map_err(|_| FractalError::invalid_input("could not parse head section"))?;
    let nodes: Vec<_> = head
        .as_node()
        .children()
        .filter(|node| node.as_element().is_some())
        .collect();
    if nodes
        .iter()
        .any(|node| element_name(node) != Some(expected))
    {
        return Err(FractalError::invalid_input(format!(
            "this section accepts only <{expected}> elements"
        )));
    }
    Ok(nodes)
}

pub(crate) fn export_reference_id(path: &str) -> String {
    format!("fractal-reference-{path}")
}

fn export_reference_href(path: &str) -> String {
    format!("#{}", export_reference_id(path))
}

fn insert_derived_export_links(
    document: &Document,
    links: &[DerivedLink],
    included_targets: &BTreeMap<String, String>,
) -> Result<()> {
    if links.is_empty() {
        return Ok(());
    }
    let root = document
        .root
        .select_first("main[data-fractal-document]")
        .or_else(|_| document.root.select_first("body"))
        .map(|node| node.as_node().clone())
        .unwrap_or_else(|_| document.root.clone());
    let text_nodes: Vec<_> = root
        .descendants()
        .filter(|node| matches!(node.data(), NodeData::Text(_)))
        .collect();
    let mut by_node: std::collections::BTreeMap<usize, Vec<&DerivedLink>> =
        std::collections::BTreeMap::new();
    for link in links {
        by_node
            .entry(link.occurrence.start.text_node)
            .or_default()
            .push(link);
    }

    for (node_index, mut matches) in by_node {
        let Some(node) = text_nodes.get(node_index) else {
            continue;
        };
        let original = node
            .as_text()
            .expect("derived link position points to a text node")
            .borrow()
            .clone();
        matches.sort_by_key(|link| link.occurrence.start.offset);
        let mut cursor = 0;
        let mut replacements = Vec::new();
        for link in matches {
            let Some(start) = utf16_offset_to_byte(&original, link.occurrence.start.offset) else {
                continue;
            };
            let Some(end) = utf16_offset_to_byte(&original, link.occurrence.end.offset) else {
                continue;
            };
            if start < cursor || end <= start || end > original.len() {
                continue;
            }
            if start > cursor {
                replacements.push(NodeRef::new_text(&original[cursor..start]));
            }
            let href = included_targets
                .get(&link.target)
                .map(|id| format!("#{id}"))
                .unwrap_or_else(|| export_reference_href(&link.target));
            replacements.push(parse_link(&href, &original[start..end])?);
            cursor = end;
        }
        if replacements.is_empty() {
            continue;
        }
        if cursor < original.len() {
            replacements.push(NodeRef::new_text(&original[cursor..]));
        }
        for replacement in replacements {
            node.insert_before(replacement);
        }
        node.detach();
    }
    Ok(())
}

fn utf16_offset_to_byte(value: &str, offset: usize) -> Option<usize> {
    if offset == 0 {
        return Some(0);
    }
    let mut utf16_offset = 0;
    for (byte, character) in value.char_indices() {
        if utf16_offset == offset {
            return Some(byte);
        }
        utf16_offset += character.len_utf16();
        if utf16_offset > offset {
            return None;
        }
    }
    (utf16_offset == offset).then_some(value.len())
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
