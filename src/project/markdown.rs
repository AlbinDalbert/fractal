use crate::project::html::{escape_html, unescape_html};

pub(super) fn markdown_to_html(default_title: &str, markdown: &str) -> (String, String) {
    let blocks = parse_markdown_blocks(markdown);
    let title = match blocks.first() {
        Some(MarkdownBlock::Heading { level: 1, text }) => text.clone(),
        _ => default_title.to_string(),
    };
    let mut body = String::new();

    for (index, block) in blocks.iter().enumerate() {
        if index == 0 && matches!(block, MarkdownBlock::Heading { level: 1, .. }) {
            continue;
        }

        if !body.is_empty() {
            body.push('\n');
            body.push_str("      ");
        }

        match block {
            MarkdownBlock::Heading { level, text } => {
                body.push_str(&format!("<h{level}>{}</h{level}>", escape_html(text)));
            }
            MarkdownBlock::Paragraph(text) => {
                body.push_str(&format!("<p>{}</p>", escape_html(text)));
            }
        }
    }

    (title, body)
}

fn parse_markdown_blocks(markdown: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            continue;
        }

        if let Some((level, text)) = parse_markdown_heading(trimmed) {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MarkdownBlock::Heading {
                level,
                text: text.to_string(),
            });
        } else {
            paragraph.push(trimmed);
        }
    }

    flush_paragraph(&mut blocks, &mut paragraph);
    blocks
}

fn parse_markdown_heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }

    let rest = &line[level..];
    rest.strip_prefix(' ')
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| (level, text))
}

fn flush_paragraph(blocks: &mut Vec<MarkdownBlock>, paragraph: &mut Vec<&str>) {
    if paragraph.is_empty() {
        return;
    }

    blocks.push(MarkdownBlock::Paragraph(paragraph.join(" ")));
    paragraph.clear();
}

pub(super) fn html_to_markdown(html: &str) -> String {
    let Some(main_start) = html.find("    <main>") else {
        return String::new();
    };
    let content_start = main_start + "    <main>".len();
    let main_end = html[content_start..]
        .find("    </main>")
        .map(|index| content_start + index)
        .unwrap_or(html.len());

    let mut markdown = String::new();
    for line in html[content_start..main_end].lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some((level, text)) = parse_html_heading(trimmed) {
            push_markdown_block(&mut markdown, &format!("{} {}", "#".repeat(level), text));
        } else if let Some(text) = parse_html_paragraph(trimmed) {
            push_markdown_block(&mut markdown, &text);
        }
    }

    markdown
}

fn parse_html_heading(line: &str) -> Option<(usize, String)> {
    for level in 1..=6 {
        let opening = format!("<h{level}>");
        let closing = format!("</h{level}>");
        if let Some(text) = line
            .strip_prefix(&opening)
            .and_then(|rest| rest.strip_suffix(&closing))
        {
            return Some((level, unescape_html(text)));
        }
    }

    None
}

fn parse_html_paragraph(line: &str) -> Option<String> {
    line.strip_prefix("<p>")
        .and_then(|rest| rest.strip_suffix("</p>"))
        .map(unescape_html)
}

fn push_markdown_block(markdown: &mut String, block: &str) {
    if !markdown.is_empty() {
        markdown.push_str("\n\n");
    }
    markdown.push_str(block);
}

#[derive(Debug, PartialEq, Eq)]
enum MarkdownBlock {
    Heading { level: usize, text: String },
    Paragraph(String),
}
