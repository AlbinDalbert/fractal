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

pub(super) fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(super) fn escape_html_attribute(input: &str) -> String {
    escape_html(input).replace('"', "&quot;")
}
