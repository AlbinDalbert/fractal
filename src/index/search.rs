use crate::document::html::find_case_insensitive;
use crate::document::PageDocument;
use crate::index::load_project_index;
use crate::project::constants::PAGES_DIR;
use crate::types::{PageEntry, SearchMatch, SearchResult};
use crate::Result;
use std::collections::BTreeSet;
use std::path::Path;

pub fn search_project(root: impl AsRef<Path>, query: &str) -> Result<Vec<SearchResult>> {
    let root = root.as_ref();
    let terms = query_terms(query)?;
    let index = load_project_index(root)?;
    let mut results = index
        .pages
        .iter()
        .filter_map(|page| {
            let body_text = searchable_page_body(root, page).unwrap_or_default();
            page_matches_all_terms(page, &body_text, &terms).then(|| SearchResult {
                path: page.path.clone(),
                title: page.title.clone(),
                matches: matching_fields(page, &body_text, &terms),
            })
        })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(results)
}

pub fn search_report(root: impl AsRef<Path>, query: &str) -> Result<String> {
    let results = search_project(root, query)?;

    let mut report = String::new();
    report.push_str(&format!("search results for `{}`\n", query.trim()));
    if results.is_empty() {
        report.push_str("  (none)\n");
        return Ok(report);
    }

    for result in results {
        report.push_str(&format!("  - {} ({})\n", result.path, result.title));
        for search_match in result.matches {
            report.push_str(&format!(
                "    {}: {}\n",
                search_match.field, search_match.text
            ));
        }
    }
    Ok(report)
}

fn query_terms(query: &str) -> Result<Vec<String>> {
    let terms = query
        .split_whitespace()
        .map(|term| term.to_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    if terms.is_empty() {
        return Err("search query cannot be empty".into());
    }

    Ok(terms)
}

fn page_matches_all_terms(page: &PageEntry, body_text: &str, terms: &[String]) -> bool {
    let haystack = searchable_page_text(page, body_text);
    terms.iter().all(|term| haystack.contains(term))
}

fn searchable_page_text(page: &PageEntry, body_text: &str) -> String {
    let mut text = vec![page.title.as_str()];
    text.extend(page.meta.values().map(String::as_str));
    text.extend(page.notes.iter().map(|note| note.label.as_str()));
    text.extend(page.links.iter().map(|link| link.text.as_str()));
    text.push(body_text);
    text.join(" ").to_lowercase()
}

fn matching_fields(page: &PageEntry, body_text: &str, terms: &[String]) -> Vec<SearchMatch> {
    let mut matches = BTreeSet::new();

    push_match(&mut matches, "title", &page.title, terms);
    if let Some(summary) = page.meta.get("fractal:summary") {
        push_match(&mut matches, "summary", summary, terms);
    }
    if let Some(tags) = page.meta.get("fractal:tags") {
        push_match(&mut matches, "tags", tags, terms);
    }

    for note in &page.notes {
        push_match(&mut matches, "note", &note.label, terms);
    }

    for link in &page.links {
        push_match(&mut matches, "link", &link.text, terms);
    }
    push_body_match(&mut matches, body_text, terms);

    matches.into_iter().collect()
}

fn searchable_page_body(root: &Path, page: &PageEntry) -> Result<String> {
    PageDocument::from_path(&root.join(PAGES_DIR).join(&page.path))?.main_text()
}

fn push_match(matches: &mut BTreeSet<SearchMatch>, field: &str, text: &str, terms: &[String]) {
    let normalized = text.to_lowercase();
    if !terms.iter().any(|term| normalized.contains(term)) {
        return;
    }

    matches.insert(SearchMatch {
        field: field.to_string(),
        text: text.to_string(),
    });
}

fn push_body_match(matches: &mut BTreeSet<SearchMatch>, text: &str, terms: &[String]) {
    let Some(position) = terms
        .iter()
        .filter_map(|term| find_case_insensitive(text, term, 0))
        .min()
    else {
        return;
    };

    matches.insert(SearchMatch {
        field: "body".to_string(),
        text: body_snippet(text, position),
    });
}

fn body_snippet(text: &str, position: usize) -> String {
    const CONTEXT: usize = 80;

    let start = text[..position]
        .char_indices()
        .rev()
        .nth(CONTEXT)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = text[position..]
        .char_indices()
        .nth(CONTEXT)
        .map(|(index, _)| position + index)
        .unwrap_or(text.len());
    let prefix = if start == 0 { "" } else { "... " };
    let suffix = if end == text.len() { "" } else { " ..." };

    format!("{prefix}{}{suffix}", text[start..end].trim())
}
