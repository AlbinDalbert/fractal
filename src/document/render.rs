use crate::document::html::escape_html;
use crate::project::constants::{DEFAULT_SUMMARY, DEFAULT_TAGS, DEFAULT_VERSION};
use crate::types::Theme;
use std::path::Path;

pub(crate) fn required_meta_tags() -> [(&'static str, &'static str); 3] {
    [
        ("fractal:version", DEFAULT_VERSION),
        ("fractal:summary", DEFAULT_SUMMARY),
        ("fractal:tags", DEFAULT_TAGS),
    ]
}

pub(crate) fn render_page_document(
    title: &str,
    body: &str,
    theme: Theme,
    stylesheet_href: String,
) -> String {
    let escaped_title = escape_html(title);
    let escaped_summary = escape_html(DEFAULT_SUMMARY);
    let escaped_tags = escape_html(DEFAULT_TAGS);
    let escaped_stylesheet_href = escape_html(&stylesheet_href);
    let theme = theme.as_str();

    format!(
        "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"utf-8\">\n    <title>{escaped_title}</title>\n    <meta name=\"fractal:version\" content=\"{DEFAULT_VERSION}\" />\n    <meta name=\"fractal:summary\" content=\"{escaped_summary}\" />\n    <meta name=\"fractal:tags\" content=\"{escaped_tags}\" />\n    <link rel=\"stylesheet\" href=\"{escaped_stylesheet_href}\">\n  </head>\n  <body data-fractal-theme=\"{theme}\">\n    <main>\n      <h1>{escaped_title}</h1>\n      {body}\n    </main>\n    <section data-fractal-notes>\n    </section>\n  </body>\n</html>\n"
    )
}

pub(crate) fn stylesheet_href(page_path: &Path) -> String {
    let parent_depth = page_path
        .parent()
        .map(|parent| parent.components().count())
        .unwrap_or(0);
    let mut href = String::new();
    for _ in 0..=parent_depth {
        href.push_str("../");
    }
    href.push_str(".fractal/style.css");
    href
}

pub(crate) fn default_stylesheet() -> &'static str {
    r#"body[data-fractal-theme="dark"] {
  --fractal-background: #19110b;
  --fractal-surface: #26180f;
  --fractal-text: #f6cb5c;
  --fractal-muted: #cca061;
  --fractal-border: #402919;
  --fractal-accent: #e4ae51;
}

body[data-fractal-theme="light"] {
  --fractal-background: #f3eadb;
  --fractal-surface: #fbf6ed;
  --fractal-text: #3a2418;
  --fractal-muted: #7f664c;
  --fractal-border: #ddceb8;
  --fractal-accent: #9a6b2f;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  background: var(--fractal-background);
  color: var(--fractal-text);
  font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  line-height: 1.6;
}

main,
section[data-fractal-notes] {
  width: min(760px, calc(100% - 32px));
  margin: 0 auto;
}

main {
  padding: 56px 0 32px;
}

h1 {
  margin: 0 0 24px;
  font-size: 2.25rem;
  line-height: 1.15;
}

a {
  color: var(--fractal-accent);
}

pre,
aside[data-fractal-note] {
  border: 1px solid var(--fractal-border);
  border-radius: 8px;
  background: var(--fractal-surface);
}

pre {
  overflow-x: auto;
  padding: 16px;
}

section[data-fractal-notes] {
  padding: 0 0 56px;
}

aside[data-fractal-note] {
  margin: 16px 0 0;
  padding: 12px 16px;
  color: var(--fractal-muted);
}
"#
}
