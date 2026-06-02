use std::path::{Component, Path};

pub(crate) fn inferred_link_scope(href: &str) -> &'static str {
    if href.starts_with('#') {
        "note"
    } else if is_external_href(href) {
        "external"
    } else {
        "page"
    }
}

pub(crate) fn note_label_from_id(note_id: &str) -> String {
    note_id
        .strip_prefix("note-")
        .unwrap_or(note_id)
        .replace('-', " ")
}

pub(crate) fn page_label_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.replace(['-', '_'], " "))
        .map(|stem| normalize_link_label(&stem))
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| path.to_string())
}

pub(crate) fn page_link_labels(path: &str, title: &str) -> [String; 2] {
    [normalize_link_label(title), page_label_from_path(path)]
}

pub(crate) fn normalize_link_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn link_label_key(label: &str) -> String {
    normalize_link_label(label).to_lowercase()
}

pub(crate) fn is_linkable_label(label: &str) -> bool {
    label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .count()
        >= 2
}

pub(crate) fn relative_href(from_page: &str, target_page: &str) -> String {
    let mut from_parts = from_page.split('/').collect::<Vec<_>>();
    if !from_parts.is_empty() {
        from_parts.pop();
    }

    let target_parts = target_page.split('/').collect::<Vec<_>>();
    let mut common = 0;
    while common < from_parts.len()
        && common < target_parts.len()
        && from_parts[common] == target_parts[common]
    {
        common += 1;
    }

    let mut href_parts = vec![".."; from_parts.len() - common];
    href_parts.extend(target_parts[common..].iter().copied());

    if href_parts.is_empty() {
        target_page.to_string()
    } else {
        href_parts.join("/")
    }
}

pub(crate) fn resolve_page_href(from_page: &str, href: &str) -> Option<String> {
    if href.starts_with('#') {
        return Some(from_page.to_string());
    }
    if is_external_href(href) || href.starts_with('/') {
        return None;
    }

    let href_without_fragment = href.split('#').next().unwrap_or(href);
    let href_path = href_without_fragment
        .split('?')
        .next()
        .unwrap_or(href_without_fragment);
    if href_path.is_empty() {
        return Some(from_page.to_string());
    }

    let base = Path::new(from_page)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    normalize_project_relative_path(&base.join(href_path))
}

pub(crate) fn normalize_project_relative_path(path: &Path) -> Option<String> {
    let mut parts = Vec::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    (!parts.is_empty()).then(|| parts.join("/"))
}

pub(crate) fn is_external_href(href: &str) -> bool {
    href.contains("://") || href.starts_with("mailto:") || href.starts_with("tel:")
}
