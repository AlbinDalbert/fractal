use crate::project::constants::PAGES_DIR;
use crate::project::document::PageDocument;
use crate::project::html::{
    escape_html, find_case_insensitive, find_element_inner_bounds, html_tag_name, is_closing_tag,
    is_self_closing_tag,
};
use crate::project::index::{build_project_index, write_generated_project_data};
use crate::project::links::{
    is_linkable_label, normalize_link_label, page_label_from_path, relative_href,
};
use crate::project::types::ProjectIndex;
use crate::Result;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn sync_project(root: impl AsRef<Path>) -> Result<()> {
    let root = root.as_ref();
    let initial_index = build_project_index(root)?;
    write_generated_project_data(root, &initial_index)?;

    let pages_dir = root.join(PAGES_DIR);
    let mut synced = 0;
    for page in &initial_index.pages {
        let path = pages_dir.join(&page.path);
        let html = fs::read_to_string(&path)?;
        let updated = sync_page_links(&html, &page.path, &initial_index)?;
        if updated != html {
            fs::write(&path, updated)?;
            synced += 1;
            println!("synced {}", path.display());
        }
    }

    let final_index = build_project_index(root)?;
    write_generated_project_data(root, &final_index)?;
    println!("sync complete: {synced} page(s) updated");
    Ok(())
}

pub(super) fn sync_page_links(html: &str, page_path: &str, index: &ProjectIndex) -> Result<String> {
    let (main_start, main_end) =
        find_element_inner_bounds(html, "main").ok_or("missing main section in page")?;

    let main = unwrap_generated_links(&html[main_start..main_end]);
    let note_candidates = note_link_candidates(html);
    let project_candidates = project_link_candidates(index, page_path);
    let main = link_candidates_in_html(&main, &note_candidates);
    let main = link_candidates_in_html(&main, &project_candidates);

    let mut updated = String::new();
    updated.push_str(&html[..main_start]);
    updated.push_str(&main);
    updated.push_str(&html[main_end..]);
    Ok(updated)
}

fn note_link_candidates(html: &str) -> Vec<LinkCandidate> {
    let document = PageDocument::parse(html);
    let mut candidates = document
        .notes()
        .into_iter()
        .filter(|note| is_linkable_label(&note.label))
        .map(|note| LinkCandidate {
            match_text: escape_html(&note.label),
            href: format!("#{}", note.id),
            scope: "note",
        })
        .collect::<Vec<_>>();

    sort_link_candidates(&mut candidates);
    candidates
}

fn project_link_candidates(index: &ProjectIndex, current_page: &str) -> Vec<LinkCandidate> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();

    for page in &index.pages {
        if page.path == current_page {
            continue;
        }

        for label in [page.title.clone(), page_label_from_path(&page.path)] {
            let label = normalize_link_label(&label);
            if !is_linkable_label(&label) {
                continue;
            }

            let key = label.to_ascii_lowercase();
            if seen.insert(key) {
                candidates.push(LinkCandidate {
                    match_text: escape_html(&label),
                    href: relative_href(current_page, &page.path),
                    scope: "page",
                });
            }
        }
    }

    sort_link_candidates(&mut candidates);
    candidates
}

fn sort_link_candidates(candidates: &mut [LinkCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .match_text
            .len()
            .cmp(&left.match_text.len())
            .then_with(|| left.match_text.cmp(&right.match_text))
    });
}

fn unwrap_generated_links(html: &str) -> String {
    let mut output = String::new();
    let mut offset = 0;

    while let Some(start) = html[offset..].find("<a") {
        let start = offset + start;
        output.push_str(&html[offset..start]);

        let Some(tag_end) = html[start..].find('>') else {
            output.push_str(&html[start..]);
            return output;
        };
        let tag_end = start + tag_end + 1;
        let tag = &html[start..tag_end];

        if html_tag_name(tag).as_deref() == Some("a") && tag.contains("data-fractal-link=\"") {
            if let Some(close_start) = html[tag_end..].find("</a>") {
                let close_start = tag_end + close_start;
                output.push_str(&html[tag_end..close_start]);
                offset = close_start + "</a>".len();
                continue;
            }
        }

        output.push_str(tag);
        offset = tag_end;
    }

    output.push_str(&html[offset..]);
    output
}

fn link_candidates_in_html(html: &str, candidates: &[LinkCandidate]) -> String {
    if candidates.is_empty() {
        return html.to_string();
    }

    let mut output = String::new();
    let mut offset = 0;
    let mut skip_depth = 0usize;

    while let Some(tag_start) = html[offset..].find('<') {
        let tag_start = offset + tag_start;
        let text = &html[offset..tag_start];
        if skip_depth == 0 {
            output.push_str(&link_text_segment(text, candidates));
        } else {
            output.push_str(text);
        }

        let Some(tag_end) = html[tag_start..].find('>') else {
            output.push_str(&html[tag_start..]);
            return output;
        };
        let tag_end = tag_start + tag_end + 1;
        let tag = &html[tag_start..tag_end];
        output.push_str(tag);
        update_link_skip_depth(tag, &mut skip_depth);
        offset = tag_end;
    }

    let text = &html[offset..];
    if skip_depth == 0 {
        output.push_str(&link_text_segment(text, candidates));
    } else {
        output.push_str(text);
    }

    output
}

fn link_text_segment(text: &str, candidates: &[LinkCandidate]) -> String {
    let mut ranges = Vec::new();

    for candidate in candidates {
        let mut search_start = 0;
        while let Some(start) = find_case_insensitive(text, &candidate.match_text, search_start) {
            let end = start + candidate.match_text.len();
            if is_phrase_boundary(text, start, end)
                && !ranges_overlap(&ranges, start, end)
                && !is_inside_html_entity(text, start, end)
            {
                ranges.push(LinkRange {
                    start,
                    end,
                    candidate: candidate.clone(),
                });
                search_start = end;
            } else {
                search_start = start + 1;
            }
        }
    }

    if ranges.is_empty() {
        return text.to_string();
    }

    ranges.sort_by_key(|range| range.start);

    let mut output = String::new();
    let mut offset = 0;
    for range in ranges {
        output.push_str(&text[offset..range.start]);
        output.push_str(&format!(
            "<a href=\"{}\" data-fractal-link=\"{}\">{}</a>",
            escape_html(&range.candidate.href),
            escape_html(range.candidate.scope),
            &text[range.start..range.end]
        ));
        offset = range.end;
    }
    output.push_str(&text[offset..]);
    output
}

fn ranges_overlap(ranges: &[LinkRange], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|range| start < range.end && end > range.start)
}

fn is_phrase_boundary(text: &str, start: usize, end: usize) -> bool {
    let previous = text[..start].chars().next_back();
    let next = text[end..].chars().next();

    !previous.map(is_link_word_character).unwrap_or(false)
        && !next.map(is_link_word_character).unwrap_or(false)
}

fn is_link_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_inside_html_entity(text: &str, start: usize, end: usize) -> bool {
    let Some(entity_start) = text[..start].rfind('&') else {
        return false;
    };
    if text[..start]
        .rfind(';')
        .map(|semicolon| semicolon > entity_start)
        .unwrap_or(false)
    {
        return false;
    }

    text[end..]
        .find(';')
        .map(|semicolon| {
            let entity = &text[entity_start..end + semicolon + 1];
            !entity.contains(char::is_whitespace)
        })
        .unwrap_or(false)
}

fn update_link_skip_depth(tag: &str, skip_depth: &mut usize) {
    let Some(name) = html_tag_name(tag) else {
        return;
    };
    if !matches!(
        name.as_str(),
        "a" | "code" | "pre" | "script" | "style" | "textarea"
    ) {
        return;
    }

    if is_closing_tag(tag) {
        *skip_depth = skip_depth.saturating_sub(1);
    } else if !is_self_closing_tag(tag) {
        *skip_depth += 1;
    }
}

#[derive(Clone)]
struct LinkCandidate {
    match_text: String,
    href: String,
    scope: &'static str,
}

struct LinkRange {
    start: usize,
    end: usize,
    candidate: LinkCandidate,
}
