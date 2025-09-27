mod parse_mark;
mod grammer;
use std::string::ParseError;

use parse_mark::parse_chunk;

pub struct MarkMrk;

impl MarkMrk {
    /// Parse .mark into the intermediate representation
    pub fn parse_mark(input: &str) -> Result<IntermediateRep, ParseError> {

        let chunks: Vec<&str> = input.split('\n').collect();

        let mut ir: IntermediateRep = IntermediateRep {
            elements: Vec::new(),
            size: 0,
            count: 0,
        };

        for chk in chunks {
            match parse_chunk(chk) {
                Ok(tvec) => {
                    ir.size += chk.len() as u64;  // Increment the size
                    ir.count += tvec.len();  // Count elements
                    ir.elements.extend(tvec);  // Append to the elements
                }
                Err(e) => {
                    eprintln!("Error parsing chunk: {:?}", e);  // Example error handling
                    return Err(e);  // Propagate the error back
                }
            }
        }

        return Ok(ir);
    }
    
    /// Serialize the IR into the `.mrk` format
    pub fn serialize(ir: &IntermediateRep, compression: CompressionLevel) -> Vec<u8> {
        todo!()
    }
    
    /// Deserialize the `.mrk` format back into the IR
    pub fn deserialize(data: &[u8]) -> IntermediateRep {
        todo!()
    }
    
    /// Export the IR back to .mark
    pub fn export_mark(ir: &IntermediateRep) -> String {
        todo!()
    }
}

pub enum CompressionLevel {
    Hamill,
    Wahlberg,
    Ruffalo,
}

pub struct IntermediateRep {
    pub size: u64,                       // Size of the document in bytes
    pub elements: Vec<DocElm>,           // Document elements
    pub count: usize,            // Number of elements
    // pub last_modified: Option<u64>,      // Unix timestamp of last modification
    // pub author: Option<String>,          // Document author
    // pub title: Option<String>,           // Document title
    // pub checksum: Option<String>,        // Checksum or hash for integrity
    // pub category: Option<String>,        // Category of the document (optional)
}

// -------- For the IR -------- //

pub enum DocElm {
    Header(Header),
    Paragraph(Paragraph),
    Citation(Citation),
    List(List),
    Image(Image),
    CodeBlock(CodeBlock),
    Table(Table),
    Field(Field),
    Break,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Style {
    Italic,
    Bold,
    Code,
    Link { href: String },
    Strikethrough,
    Highlight,
}


pub struct Header {
    pub level: u8,
    pub text: Span,
}

impl Header {
    pub fn new() -> Self {
        Header {
            level: 1,
            text: Span::new(),
        }
    }

    pub fn push(&mut self, c: char) {
        self.text.push(c);
    }

    pub fn append(&mut self, str: String) {
        self.text.append(str);
    }
}

pub struct Paragraph {
    pub text: Vec<Span>,
}

impl Paragraph {
    pub fn new() -> Self {
        Paragraph {
            text: Vec::new(),
        }
    }
}

pub struct Citation {
    pub text: Vec<Span>,
    pub src: Option<String>,
    pub date: Option<String>,
}

impl Citation {
    pub fn new() -> Self {
        Citation {
            text: Vec::new(),
            src: None,
            date: None,
        }
    }
}

pub struct List {
    pub items: Vec<Span>,
    pub checkboxes: Option<Vec<bool>>,
}


pub struct Span {
    pub text: String,
    pub styles: Vec<Style>,
    pub footnote: Option<String>,
    pub checkbox: Option<bool>,
    pub field: Option<Field>,
}

pub enum Field { //blank input field, if span has a Field, the text content of the Span is instead a hint for what to input.
    Inline {max_length: Option<usize>},
    Multiline {max_lines: Option<usize>},
}

impl Span {
    /// Creates a new `Span` with default values for styles, hover, and checkbox.
    pub fn new() -> Self {
        Span {
            text: "".to_string(),
            styles: Vec::new(),
            footnote: None,
            checkbox: None,
            field: None,
        }
    }

    pub fn push(&mut self, c: char) {
        self.text.push(c);
    }

    pub fn append(&mut self, str: String) {
        self.text.push_str(&str);
    }
}

pub struct Image {
    pub title: String,
    pub source: String,
    pub decription: String,
}

pub struct Table {
    pub title: String,
    pub headers: Vec<Span>,
    pub rows: Vec<Vec<Span>>,
}

pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
}