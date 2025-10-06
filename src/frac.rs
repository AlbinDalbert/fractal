pub struct IntermediateRep {
    pub size: u64,                          // Size of the document in bytes
    pub body: Vec<DocElm>,                  // Document elements
    pub count: usize,                       // Number of elements
    pub last_modified: Option<u64>,         // Unix timestamp of last modification
    pub author: Option<String>,             // Document author
    pub title: String,                      // Document title
    pub checksum: Option<String>,           // Checksum or hash for integrity
    pub tags: Option<Vec<String>>,          // Category of the document
    pub footnotes: Option<Vec<Footnote>>    // Footnotes (internal links)
}

// -------- For the IR -------- //

pub struct Footnote {
    pub title: String,
    pub body: Vec<DocElm>
}

pub enum DocElm {
    Header(Header),
    Paragraph(Paragraph),
    Citation(Citation),
    List(List),
    Image(Image),
    CodeBlock(CodeBlock),
    Table(Table),
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


pub struct Paragraph {
    pub text: Vec<Span>,
}


pub struct Citation {
    pub text: Vec<Span>,
    pub src: Option<String>,
    pub date: Option<String>,
}


pub struct List {
    pub items: Vec<Span>,
    pub checkboxes: Option<Vec<bool>>,
}


pub struct Span {
    pub text: String,
    pub styles: Vec<Style>,
    pub footnote: Option<String>,
    pub checkbox: Option<bool>
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

// --- impl --- //

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

impl Span {
    /// Creates a new `Span` with default values for styles, hover, and checkbox.
    pub fn new() -> Self {
        Span {
            text: "".to_string(),
            styles: Vec::new(),
            footnote: None,
            checkbox: None
        }
    }
    
    pub fn push(&mut self, c: char) {
        self.text.push(c);
    }
    
    pub fn append(&mut self, str: String) {
        self.text.push_str(&str);
    }
}

impl Paragraph {
    pub fn new() -> Self {
        Paragraph {
            text: Vec::new(),
        }
    }
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