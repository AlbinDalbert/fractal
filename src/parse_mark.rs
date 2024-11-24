use std::string::ParseError;

use crate::*;

pub fn parse_chunk(chunk: &str) -> Result<Vec<DocElm>, ParseError> {
    let elements = doc_elemizer(chunk);



    return Ok(elements);
}

fn doc_elemizer(input: &str) -> Vec<DocElm> {
    let mut doc_elements = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            grammer::HEADER         => doc_elements.push(header_parse(&mut chars)),
            grammer::CITATION       => doc_elements.push(citation_parse(&mut chars)),
            // grammer::CODE_MARKER    => doc_elements.push(code_block_parse(&mut chars)),  
            _                       => doc_elements.push(paragraph_parse(&mut chars)),
        }
    }

    doc_elements
}

fn header_parse(chars: &mut std::iter::Peekable<std::str::Chars>) -> DocElm{
    let mut elm: DocElm = DocElm::Header(Header::new());
            
    if let DocElm::Header(ref mut header) = elm {
        while let Some(&nxt) = chars.peek() {
            if nxt == grammer::HEADER {
                header.level += 1;
                chars.next();
                continue;
            } 
            break;
        }
        
        header.text.append(chars.by_ref().take_while(|&c| c != '\n').collect::<String>().trim().to_string());
    };

    elm
}

fn paragraph_parse(chars: &mut std::iter::Peekable<std::str::Chars>) -> DocElm { 
    let mut elm: DocElm = DocElm::Paragraph(Paragraph::new());
            
    if let DocElm::Paragraph(ref mut paragraph) = elm {       
        paragraph.text.extend(span_parse(chars, None));
    };

    elm
}

fn citation_parse(chars:  &mut std::iter::Peekable<std::str::Chars>) -> DocElm {
    let mut elm: DocElm = DocElm::Citation(Citation::new());
    let styles = vec![
        Style::Italic,
    ];

    if let DocElm::Citation(ref mut citation) = elm {
        citation.text.extend(span_parse(chars, Some(styles)));
    }
    elm
}

fn span_parse(chars: &mut std::iter::Peekable<std::str::Chars>, styles: Option<Vec<Style>>) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    
    let mut current_styles = styles.unwrap_or_default();

    while let Some(&ch) = chars.peek() {
        match ch {
            '\n' => {
                // End of the current element (e.g., paragraph)
                break;
            }
            '_' => {
                // Italic marker (`__`)
                chars.next(); // Consume '_'
                if let Some(&next) = chars.peek() {
                    if next == '_' {
                        chars.next(); // Consume second '_'
                        if !current_text.is_empty() {
                            spans.push(Span {
                                text: current_text.clone(),
                                styles: current_styles.clone(),
                                footnote: None,
                                checkbox: None,
                                field: None,
                            });
                            current_text.clear();
                        }
                        // Toggle italic
                        if current_styles.contains(&Style::Italic) {
                            current_styles.retain(|style| style != &Style::Italic);
                        } else {
                            current_styles.push(Style::Italic);
                        }
                        continue;
                    } else {
                        // Single `_` - treat as normal text
                        current_text.push('_');
                        continue;
                    }
                }
            }
            '*' => {
                // Bold marker (`**`)
                chars.next(); // Consume '*'
                if let Some(&next) = chars.peek() {
                    if next == '*' {
                        chars.next(); // Consume second '*'
                        if !current_text.is_empty() {
                            spans.push(Span {
                                text: current_text.clone(),
                                styles: current_styles.clone(),
                                footnote: None,
                                checkbox: None,
                                field: None,
                            });
                            current_text.clear();
                        }
                        // Toggle bold
                        if current_styles.contains(&Style::Bold) {
                            current_styles.retain(|style| style != &Style::Bold);
                        } else {
                            current_styles.push(Style::Bold);
                        }
                        continue;
                    } else {
                        // Single '*' - treat as normal text
                        current_text.push('*');
                        continue;
                    }
                }
            }
            _ => {
                // Normal text
                current_text.push(ch);
                chars.next(); // Consume the character
            }
        }
    }

    // Push the final chunk as a span
    if !current_text.is_empty() {
        spans.push(Span {
            text: current_text,
            styles: current_styles,
            footnote: None,
            checkbox: None,
            field: None,
        });
    }

    spans
}

#[test]
fn test_parse_paragraph_nested_styling_italic_bold() {
    let mut input = "__This italic text includes a **bold** word";
    let paragraph = span_parse(&mut input.chars().peekable(), None);

    for span in paragraph {
        println!("{:?}, {:?}", span.text, span.styles);
    }
}

