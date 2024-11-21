use std::string::ParseError;
use std::iter::FromIterator;

use crate::*;

pub fn parse_chunk(chunk: &str) -> Result<Vec<DocElm>, ParseError> {
    let elements: Vec<DocElm> = Vec::new();



    return Ok(elements);
}

fn DocElemizer(input: &str) -> Vec<DocElm> {
    let mut doc_elements = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            grammer::HEADER => {
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
                }
                doc_elements.push(elm);
            },
            _ => todo!(),
        }
    }

    doc_elements
}