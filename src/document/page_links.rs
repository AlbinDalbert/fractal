use crate::document::page::PageDocument;
use crate::graph::links::{
    inferred_link_scope, is_external_href, normalize_link_label, page_link_text_matches,
    relative_href, resolve_page_href,
};
use crate::types::LinkEntry;
use brik::NodeRef;
use std::collections::BTreeMap;

impl PageDocument {
    pub(crate) fn repair_invalid_links(
        &self,
        page_path: &str,
        known_page_titles: &BTreeMap<String, String>,
    ) -> usize {
        let links = self
            .document
            .select("a[href]")
            .expect("static selector should parse")
            .map(|element| element.as_node().clone())
            .collect::<Vec<_>>();
        let mut repaired = 0;

        for link in links {
            let Some(element) = link.as_element() else {
                continue;
            };
            let attributes = element.attributes.borrow();
            let href = attributes.get("href").unwrap_or_default().to_string();
            let scope = attributes.get("data-fractal-link").map(str::to_string);
            drop(attributes);

            let text = normalize_link_label(&link.text_contents());
            let target = (!href.starts_with('#') && !is_external_href(&href))
                .then(|| resolve_page_href(page_path, &href))
                .flatten()
                .and_then(|target_path| {
                    known_page_titles
                        .get(&target_path)
                        .map(|title| (target_path, title.clone()))
                });

            match (scope.as_deref(), target.as_ref()) {
                (Some("page"), Some((target_path, title)))
                    if !page_link_text_matches(target_path, title, &text) =>
                {
                    replace_children_with_text(&link, title);
                    repaired += 1;
                }
                (None, _) => {
                    unwrap_link_node(&link, None);
                    repaired += 1;
                }
                _ => {}
            }
        }

        repaired
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

    pub(crate) fn rewrite_page_link_text(
        &self,
        from_page: &str,
        target_page: &str,
        title: &str,
    ) -> usize {
        let mut updated = 0;

        for element in self
            .document
            .select("a[href][data-fractal-link=page]")
            .expect("static selector should parse")
        {
            let attributes = element.attributes.borrow();
            let Some(href) = attributes.get("href") else {
                continue;
            };

            if resolve_page_href(from_page, href).as_deref() != Some(target_page) {
                continue;
            }

            let text = normalize_link_label(&element.text_contents());
            if page_link_text_matches(target_page, title, &text) {
                continue;
            }

            replace_children_with_text(element.as_node(), title);
            updated += 1;
        }

        updated
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

    pub(crate) fn unwrap_generated_page_hrefs(&self, from_page: &str, target_page: &str) -> usize {
        let links = self
            .document
            .select("a[href]")
            .expect("static selector should parse")
            .filter(|element| {
                let attributes = element.attributes.borrow();
                attributes.get("data-fractal-link") == Some("page")
                    && attributes
                        .get("href")
                        .and_then(|href| resolve_page_href(from_page, href))
                        .as_deref()
                        == Some(target_page)
            })
            .map(|element| element.as_node().clone())
            .collect::<Vec<_>>();
        let count = links.len();

        for link in links {
            let children = link.children().collect::<Vec<_>>();
            for child in children {
                link.insert_before(child);
            }
            link.detach();
        }

        count
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
}

fn unwrap_link_node(link: &NodeRef, suffix: Option<&str>) {
    let children = link.children().collect::<Vec<_>>();
    for child in children {
        link.insert_before(child);
    }
    if let Some(suffix) = suffix {
        link.insert_before(NodeRef::new_text(suffix));
    }
    link.detach();
}

fn replace_children_with_text(node: &NodeRef, text: &str) {
    for child in node.children().collect::<Vec<_>>() {
        child.detach();
    }
    node.append(NodeRef::new_text(text));
}

fn rewrite_href_path(href: &str, path: &str) -> String {
    let suffix_start = href
        .char_indices()
        .find_map(|(index, character)| matches!(character, '?' | '#').then_some(index))
        .unwrap_or(href.len());
    format!("{}{}", path, &href[suffix_start..])
}
