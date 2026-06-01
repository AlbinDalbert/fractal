use crate::Result;

pub(super) fn find_case_insensitive(haystack: &str, needle: &str, start: usize) -> Option<usize> {
    if needle.is_empty() || start > haystack.len() || needle.len() > haystack.len() {
        return None;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    for index in start..=haystack.len() - needle.len() {
        let end = index + needle.len();
        if !haystack.is_char_boundary(index) || !haystack.is_char_boundary(end) {
            continue;
        }

        if haystack_bytes[index..end].eq_ignore_ascii_case(needle_bytes) {
            return Some(index);
        }
    }

    None
}

pub(super) fn find_element_inner_bounds(html: &str, element: &str) -> Option<(usize, usize)> {
    let open_pattern = format!("<{element}");
    let open_start = html.find(&open_pattern)?;
    let open_end = html[open_start..].find('>')? + open_start + 1;
    let close_pattern = format!("</{element}>");
    let close_start = html[open_end..].find(&close_pattern)? + open_end;

    Some((open_end, close_start))
}

pub(super) fn html_tag_name(tag: &str) -> Option<String> {
    let inner = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .trim_end_matches('/')
        .trim();
    if inner.starts_with('!') || inner.starts_with('?') {
        return None;
    }

    let inner = inner.strip_prefix('/').unwrap_or(inner).trim_start();
    let name = inner
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

pub(super) fn is_closing_tag(tag: &str) -> bool {
    tag.trim_start_matches('<').trim_start().starts_with('/')
}

pub(super) fn is_self_closing_tag(tag: &str) -> bool {
    tag.trim_end().ends_with("/>")
}

pub(super) fn insert_before_head_close(html: &str, insertion: &str) -> Result<String> {
    let Some(index) = find_case_insensitive(html, "</head>", 0) else {
        return Err("missing head close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

pub(super) fn insert_before_body_close(html: &str, insertion: &str) -> Result<String> {
    let Some(index) = find_case_insensitive(html, "</body>", 0) else {
        return Err("missing body close tag in page".into());
    };

    let mut updated = String::new();
    updated.push_str(&html[..index]);
    updated.push_str(insertion);
    updated.push_str(&html[index..]);
    Ok(updated)
}

pub(super) fn insert_body_attribute(html: &str, attribute: &str) -> Option<String> {
    let body_start = find_case_insensitive(html, "<body", 0)?;
    let body_end = html[body_start..].find('>')? + body_start;

    let mut updated = String::new();
    updated.push_str(&html[..body_end]);
    updated.push(' ');
    updated.push_str(attribute);
    updated.push_str(&html[body_end..]);
    Some(updated)
}

pub(super) fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn unescape_html(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
