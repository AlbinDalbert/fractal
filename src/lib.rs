use std::fmt::LowerHex;

pub struct MiniMark;

impl MiniMark {
    /// Parse .mark into the intermediate representation
    pub fn parse_mark(input: &str) -> IntermediateRep {
        todo!()
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
    pub elements: Vec<DocElm>,
}

// -------- For the IR -------- //

enum DocElm {
    Header(Header),
    Paragraph(Paragraph),
    List(List),
    Image(Image),
    CodeBlock(CodeBlock),
    Table(Table),
    Break,
}

#[derive(Debug, Clone)]
pub enum Style {
    Italic,
    Bold,
    Code,
    Color(Color),
    Link { href: String },
    Strikethrough,
    Highlight,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Hex(String),
    //Named(String),
    //Rgb(u8, u8, u8),
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
    language: Option<String>,
    code: String,
}

pub struct Header {
    pub level: u8,
    pub text: Span,
}

pub struct Paragraph {
    pub text: Span,
}

pub struct List {
    pub items: Vec<Span>,
}

pub struct Span {
    pub text: String,
    pub styles: Vec<Style>,
    pub hover: Option<String>,
}

