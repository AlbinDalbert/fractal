use crate::document::html::escape_html;
use crate::document::PageDocument;
use brik::NodeRef;

pub(crate) fn markdown_to_html(default_title: &str, markdown: &str) -> (String, String) {
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

pub(crate) fn html_to_markdown(html: &str) -> String {
    let document = PageDocument::parse(html);
    let Ok(main) = document.document.select_first("main") else {
        return String::new();
    };

    let mut markdown = String::new();
    for node in main.as_node().descendants() {
        if let Some(block) = markdown_block_from_node(&node) {
            push_markdown_block(&mut markdown, &block);
        }
    }

    markdown
}

fn markdown_block_from_node(node: &NodeRef) -> Option<String> {
    let element = node.as_element()?;
    let tag = element.name.local.to_string();
    let text = node.text_contents();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }

    if tag == "p" {
        return Some(text);
    }

    if let Some(level) = tag
        .strip_prefix('h')
        .and_then(|level| level.parse::<usize>().ok())
    {
        if (1..=6).contains(&level) {
            return Some(format!("{} {}", "#".repeat(level), text));
        }
    }

    None
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
