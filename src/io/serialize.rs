use std::io::{Write, Cursor};
use byteorder::{LittleEndian, WriteBytesExt};
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

// -- CORE SERIALIZATION LOGIC --

pub fn serialize_ir(ir: &IntermediateRep) -> Result<Vec<u8>, FracFormatError> {
    let mut body_buf = Vec::new();
    for elm in &ir.body {
        write_doc_elm(&mut body_buf, elm)?;
    }

    let mut footnotes_buf = Vec::new();
    let footnotes = ir.footnotes.as_deref().unwrap_or(&[]);
    write_footnotes(&mut footnotes_buf, footnotes)?;

    let body_offset = HEADER_SIZE as u32;
    let footnotes_offset = body_offset + body_buf.len() as u32;

    let mut checksum_hasher = crc32fast::Hasher::new();
    checksum_hasher.update(&body_buf);
    checksum_hasher.update(&footnotes_buf);
    let checksum = checksum_hasher.finalize();

    let mut final_bytes = Vec::with_capacity(HEADER_SIZE + body_buf.len() + footnotes_buf.len());
    let mut header_cursor = Cursor::new(&mut final_bytes);

    // --- Write Header ---
    header_cursor.write_all(MAGIC_BYTES)?;
    header_cursor.write_u32::<LittleEndian>(checksum)?;
    header_cursor.write_u8(FORMAT_VERSION)?;
    header_cursor.write_u32::<LittleEndian>(body_offset)?;
    header_cursor.write_u32::<LittleEndian>(footnotes_offset)?;
    header_cursor.write_all(&vec![0; HEADER_RESERVED_BYTES])?; // Reserved bytes

    // --- Append Body and Footnotes ---
    final_bytes.extend_from_slice(&body_buf);
    final_bytes.extend_from_slice(&footnotes_buf);

    Ok(final_bytes)
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

fn write_doc_elm(w: &mut impl Write, elm: &DocElm) -> Result<(), FracFormatError> {
    match elm {
        DocElm::Header(h) => {
            w.write_u8(DocElmTag::Header as u8)?;
            w.write_u8(h.level)?;
            write_span(w, &h.text)?;
        }
        DocElm::Paragraph(p) => {
            w.write_u8(DocElmTag::Paragraph as u8)?;
            write_span_vec(w, &p.text)?;
        }
        DocElm::Citation(c) => {
            w.write_u8(DocElmTag::Citation as u8)?;
            write_span_vec(w, &c.text)?;
            write_option_string(w, &c.src)?;
            write_option_string(w, &c.date)?;
        }
        DocElm::List(l) => {
            w.write_u8(DocElmTag::List as u8)?;
            write_span_vec(w, &l.items)?;
            match &l.checkboxes {
                Some(checkboxes) => {
                    w.write_u8(1)?;
                    w.write_u64::<LittleEndian>(checkboxes.len() as u64)?;
                    for checkbox in checkboxes {
                        w.write_u8(if *checkbox { 1 } else { 0 })?;
                    }
                }
                None => w.write_u8(0)?,
            }
        }
        DocElm::Image(i) => {
            w.write_u8(DocElmTag::Image as u8)?;
            write_string(w, &i.title)?;
            write_string(w, &i.source)?;
            write_string(w, &i.decription)?;
        }
        DocElm::CodeBlock(cb) => {
            w.write_u8(DocElmTag::CodeBlock as u8)?;
            write_option_string(w, &cb.language)?;
            write_string(w, &cb.code)?;
        }
        DocElm::Table(t) => {
            w.write_u8(DocElmTag::Table as u8)?;
            write_string(w, &t.title)?;
            write_span_vec(w, &t.headers)?;
            w.write_u64::<LittleEndian>(t.rows.len() as u64)?;
            for row in &t.rows {
                write_span_vec(w, row)?;
            }
        }
        DocElm::Break => {
            w.write_u8(DocElmTag::Break as u8)?;
        }
    }
    Ok(())
}

fn write_footnotes(w: &mut impl Write, footnotes: &[Footnote]) -> Result<(), FracFormatError> {
    w.write_u64::<LittleEndian>(footnotes.len() as u64)?;
    for note in footnotes {
        write_string(w, &note.id)?;
        write_string(w, &note.title)?;
        w.write_u64::<LittleEndian>(note.body.len() as u64)?;
        for elm in &note.body {
            write_doc_elm(w, elm)?;
        }
    }
    Ok(())
}

fn write_span(w: &mut impl Write, span: &Span) -> Result<(), FracFormatError> {
    write_string(w, &span.text)?;
    w.write_u64::<LittleEndian>(span.styles.len() as u64)?;
    
    for style in &span.styles {
        match style {
            Style::Italic => w.write_u8(StyleTag::Italic as u8)?,
            Style::Bold => w.write_u8(StyleTag::Bold as u8)?,
            Style::Code => w.write_u8(StyleTag::Code as u8)?,
            Style::Link { href } => {
                w.write_u8(StyleTag::Link as u8)?;
                write_string(w, href)?;
            }
            Style::Strikethrough => w.write_u8(StyleTag::Strikethrough as u8)?,
            Style::Highlight => w.write_u8(StyleTag::Highlight as u8)?,
        }
    }

    if span.footnote.is_some() {
        w.write_u8(1)?;
        write_string(w, span.footnote.as_ref().unwrap())?;
    } else {
        w.write_u8(0)?;
    }

    if span.checkbox.is_some() {
        w.write_u8(1)?;
        w.write_u8(if span.checkbox.unwrap() { 1 } else { 0 })?;
    } else {
        w.write_u8(0)?;
    }

    Ok(())
}

fn write_span_vec(w: &mut impl Write, spans: &[Span]) -> Result<(), FracFormatError> {
    w.write_u64::<LittleEndian>(spans.len() as u64)?;
    for span in spans {
        write_span(w, span)?;
    }
    Ok(())
}

fn write_string(w: &mut impl Write, s: &str) -> Result<(), FracFormatError> {
    let bytes = s.as_bytes();
    w.write_u64::<LittleEndian>(bytes.len() as u64)?;
    w.write_all(bytes)?;
    Ok(())
}

fn write_option_string(w: &mut impl Write, value: &Option<String>) -> Result<(), FracFormatError> {
    match value {
        Some(v) => {
            w.write_u8(1)?;
            write_string(w, v)?;
        }
        None => w.write_u8(0)?,
    }
    Ok(())
}
