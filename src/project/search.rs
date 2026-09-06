use super::support::*;
use super::*;

impl Project {
    /// Searches native document titles and visible text for all
    /// whitespace-separated terms, ignoring case.
    ///
    /// An empty query returns no results. Search reads the in-memory native
    /// document catalog and does not write project files.
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
            .filter(|stored| words.iter().all(|word| stored.search_text.contains(word)))
            .map(|stored| SearchResult {
                path: stored.page.path.clone(),
                title: stored.page.title.clone(),
                snippet: snippet(&stored.page.text, &words[0]),
            })
            .collect()
    }

    /// Finds unambiguous, case-insensitive exact-title matches in unlinked
    /// native document text without changing source.
    pub fn derived_links(&self, path: impl AsRef<Path>) -> Result<Vec<DerivedLink>> {
        let source = self.stored(path.as_ref())?;
        let mut titles: Vec<_> = self
            .title_index
            .values()
            .filter_map(|targets| {
                let targets: Vec<_> = targets
                    .iter()
                    .filter(|(path, _)| path != &source.page.path)
                    .collect();
                match targets.as_slice() {
                    [(path, title)] => Some((path.as_str(), title.as_str())),
                    _ => None,
                }
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
}
