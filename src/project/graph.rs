use super::support::*;
use super::*;

impl Project {
    pub fn links(&self, path: impl AsRef<Path>) -> Result<Vec<Link>> {
        Ok(self.stored(path.as_ref())?.page.links.clone())
    }

    pub fn iframes(&self, path: impl AsRef<Path>) -> Result<Vec<Iframe>> {
        Ok(self.stored(path.as_ref())?.page.iframes.clone())
    }

    pub fn backlinks(&self, path: impl AsRef<Path>) -> Result<Vec<Backlink>> {
        let target = path_string(&self.existing_path(path.as_ref())?);
        let mut backlinks = Vec::new();
        for page in self.pages.values() {
            for link in &page.page.links {
                if matches!(&link.target, LinkTarget::Internal(value) if value == &target) {
                    backlinks.push(Backlink {
                        page: page.page.path.clone(),
                        text: link.text.clone(),
                    });
                }
            }
        }
        Ok(backlinks)
    }

    pub fn iframe_backlinks(&self, path: impl AsRef<Path>) -> Result<Vec<IframeBacklink>> {
        let target = path_string(&self.existing_path(path.as_ref())?);
        let mut backlinks = Vec::new();
        for page in self.pages.values() {
            for iframe in &page.page.iframes {
                if matches!(&iframe.target, IframeTarget::Internal(value) if value == &target) {
                    backlinks.push(IframeBacklink {
                        page: page.page.path.clone(),
                        title: iframe.title.clone(),
                    });
                }
            }
        }
        Ok(backlinks)
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let words: Vec<String> = query
            .split_whitespace()
            .map(|word| word.to_lowercase())
            .collect();
        if words.is_empty() {
            return vec![];
        }
        self.pages
            .values()
            .filter_map(|stored| {
                let haystack = format!(
                    "{} {}",
                    stored.page.title.as_deref().unwrap_or(""),
                    stored.page.text
                )
                .to_lowercase();
                words
                    .iter()
                    .all(|word| haystack.contains(word))
                    .then(|| SearchResult {
                        path: stored.page.path.clone(),
                        title: stored.page.title.clone(),
                        snippet: snippet(&stored.page.text, &words[0]),
                    })
            })
            .collect()
    }

    /// Finds unambiguous, case-insensitive exact-title matches without changing source.
    pub fn derived_links(&self, path: impl AsRef<Path>) -> Result<Vec<DerivedLink>> {
        let source = self.stored(path.as_ref())?;
        let mut titles: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
        for target in self.pages.values() {
            if target.page.path == source.page.path {
                continue;
            }
            let Some(title) = target.page.title.as_deref() else {
                continue;
            };
            titles
                .entry(title.to_lowercase())
                .or_default()
                .push((&target.page.path, title));
        }
        let mut titles: Vec<_> = titles
            .into_values()
            .filter_map(|targets| match targets.as_slice() {
                [(path, title)] => Some((*path, *title)),
                _ => None,
            })
            .collect();
        titles.sort_by(|(left_path, left_title), (right_path, right_title)| {
            right_title
                .chars()
                .count()
                .cmp(&left_title.chars().count())
                .then_with(|| left_path.cmp(right_path))
        });

        let document = Document::parse(&source.html);
        let mut links = Vec::new();
        for node in document.unlinked_text_nodes() {
            let mut matches = Vec::new();
            for (target, title) in &titles {
                for (start, end) in exact_case_insensitive_matches(&node.text, title) {
                    matches.push((start, end, *target));
                }
            }
            matches.sort_by(
                |(left_start, left_end, left_target), (right_start, right_end, right_target)| {
                    left_start
                        .cmp(right_start)
                        .then_with(|| (right_end - right_start).cmp(&(left_end - left_start)))
                        .then_with(|| left_target.cmp(right_target))
                },
            );

            let mut claimed_until = 0;
            for (start, end, target) in matches {
                if start < claimed_until {
                    continue;
                }
                links.push(DerivedLink {
                    text: node.text[start..end].to_string(),
                    target: target.to_string(),
                    occurrence: TextOccurrence {
                        start: TextPosition {
                            text_node: node.index,
                            offset: node.text[..start].encode_utf16().count(),
                        },
                        end: TextPosition {
                            text_node: node.index,
                            offset: node.text[..end].encode_utf16().count(),
                        },
                    },
                });
                claimed_until = end;
            }
        }
        Ok(links)
    }

    pub fn insert_link(
        &mut self,
        page: impl AsRef<Path>,
        text: &str,
        target: impl AsRef<Path>,
    ) -> Result<Mutation> {
        let _lock = self.lock_for_mutation()?;
        self.reload()?;
        let page = self.existing_path(page.as_ref())?;
        let target = self.existing_path(target.as_ref())?;
        if page_kind(&page) != PageKind::Native {
            return Err(FractalError::invalid_input(
                "semantic link insertion is only available for native documents",
            ));
        }
        if page == target {
            return Err(FractalError::invalid_input("cannot link a page to itself"));
        }
        let stored = self.stored(&page)?.clone();
        let document = Document::parse(&stored.html);
        let href = relative_href(&path_string(&page), &path_string(&target));
        if !document.insert_link(text, &href)? {
            return Err(FractalError::not_found(format!(
                "unlinked text not found: {text}"
            )));
        }
        atomic_write(&self.root.join(PAGES).join(&page), &document.serialize()?)?;
        self.reload()?;
        Ok(Mutation {
            changed: vec![page],
            deleted: vec![],
        })
    }
}
