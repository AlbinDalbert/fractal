use std::io::{Cursor, Read, Seek, SeekFrom};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::formats::frac::*;
use crate::errors::FracFormatError;

// --- Constants ---
const MAGIC_BYTES: &[u8; 4] = b"FRAC";
const FORMAT_VERSION: u8 = 1;
const HEADER_SIZE: usize = 37;
const HEADER_RESERVED_BYTES: usize = 20;

// --- Tag Enums for DocElm Variants ---
#[repr(u8)]
enum DocElmTag {
    Header = 1,
    Paragraph = 2,
    Citation = 3,
    List = 4,
    Image = 5,
    CodeBlock = 6,
    Table = 7,
    Break = 8,
}

// -- CORE DESERIALIZATION LOGIC --

pub fn deserialize_ir(data: &[u8]) -> Result<IntermediateRep, FracFormatError> {
    let mut cursor = Cursor::new(data);

    // --- Read and Validate Header ---
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if magic != *MAGIC_BYTES {
        return Err(FracFormatError::InvalidHeader("Invalid magic bytes".to_string()));
    }

    let checksum_from_header = cursor.read_u32::<LittleEndian>()?;
    let version = cursor.read_u8()?;
    if version != FORMAT_VERSION {
        return Err(FracFormatError::InvalidHeader(format!("Unsupported version: {}", version)));
    }

    let body_offset = cursor.read_u32::<LittleEndian>()? as usize;
    let footnotes_offset = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.seek(SeekFrom::Current(HEADER_RESERVED_BYTES as i64))?;

    // --- Verify Checksum ---
    let body_slice = &data[body_offset..footnotes_offset];
    let footnotes_slice = &data[footnotes_offset..];
    let mut checksum_hasher = crc32fast::Hasher::new();
    checksum_hasher.update(body_slice);
    checksum_hasher.update(footnotes_slice);
    let calculated_checksum = checksum_hasher.finalize();

    if checksum_from_header != calculated_checksum {
        return Err(FracFormatError::InvalidData("Checksum mismatch".to_string()));
    }

    // --- Deserialize Body ---
    cursor.seek(SeekFrom::Start(body_offset as u64))?;
    let mut body = Vec::new();
    while cursor.position() < footnotes_offset as u64 {
        body.push(read_doc_elm(&mut cursor)?);
    }

    // --- Deserialize Footnotes ---
    cursor.seek(SeekFrom::Start(footnotes_offset as u64))?;
    let mut footnotes_vec = Vec::new();
    let num_footnotes = cursor.read_u64::<LittleEndian>()?;
    for _ in 0..num_footnotes {
        let title = read_string(&mut cursor)?;
        let num_body_elms = cursor.read_u64::<LittleEndian>()?;
        let mut note_body = Vec::new();
        for _ in 0..num_body_elms {
            note_body.push(read_doc_elm(&mut cursor)?);
        }
        footnotes_vec.push(Footnote { title, body: note_body });
    }
    let footnotes = if num_footnotes == 0 {
        None
    } else {
        Some(footnotes_vec)
    };

    // --- Assemble IR ---
    // Note: Title, author etc. are not stored in the file format yet.
    Ok(IntermediateRep {
        body,
        footnotes,
        count: 0, // Can be recalculated if needed
        size: data.len() as u64,
        title: "".to_string(),
        author: None,
        last_modified: None,
        checksum: Some(format!("{:x}", calculated_checksum)),
        tags: None,
    })
}

// --- Helpers ---

enum StyleTag {
    Italic = 1,
    Bold = 2,
    Code = 3,
    Link = 4,
    Strikethrough = 5,
    Highlight = 6,
}

fn read_doc_elm(r: &mut impl Read) -> Result<DocElm, FracFormatError> {
    let tag = r.read_u8()?;
    match tag {
        tag if tag == DocElmTag::Header as u8 => {
            let level = r.read_u8()?;
            let text = read_span(r)?;
            Ok(DocElm::Header(Header { level, text }))
        }
        tag if tag == DocElmTag::Paragraph as u8 => {
            let text = read_span_vec(r)?;
            Ok(DocElm::Paragraph(Paragraph { text }))
        }
        _ => Err(FracFormatError::InvalidData(format!("Unknown DocElm tag: {}", tag)))
    }
}

fn read_span(r: &mut impl Read) -> Result<Span, FracFormatError> {
    let text = read_string(r)?;
    let styles_count = r.read_u64::<LittleEndian>()?;
    let mut styles = Vec::with_capacity(styles_count as usize);
    for _ in 0..styles_count {
        let style_tag = r.read_u8()?;
        let style = match style_tag {
            x if x == StyleTag::Italic as u8 => Style::Italic,
            x if x == StyleTag::Bold as u8 => Style::Bold,
            x if x == StyleTag::Code as u8 => Style::Code,
            x if x == StyleTag::Strikethrough as u8 => Style::Strikethrough,
            x if x == StyleTag::Highlight as u8 => Style::Highlight,
            x if x == StyleTag::Link as u8 => {
                let href = read_string(r)?;
                Style::Link { href }
            }
            other => return Err(FracFormatError::InvalidData(format!("Unknown style tag {}", other))),
        };
        styles.push(style);
    }

    let has_footnote = r.read_u8()?;
    let footnote = if has_footnote == 1 {
        Some(read_string(r)?)
    } else {
        None
    };

    let has_checkbox = r.read_u8()?;

    let checkbox = if has_checkbox == 1 {
        Some(r.read_u8()? == 1)
    } else {
        None
    };

    Ok(Span { text, footnote, checkbox, styles})
}

fn read_span_vec(r: &mut impl Read) -> Result<Vec<Span>, FracFormatError> {
    let count = r.read_u64::<LittleEndian>()?;
    let mut spans = Vec::with_capacity(count as usize);
    for _ in 0..count {
        spans.push(read_span(r)?);
    }
    Ok(spans)
}

fn read_string(r: &mut impl Read) -> Result<String, FracFormatError> {
    let len = r.read_u64::<LittleEndian>()? as usize;
    let mut buf = vec![0; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| FracFormatError::InvalidData(e.to_string()))
}
