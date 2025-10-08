use std::io::{Write, Cursor, Read, Seek, SeekFrom};
use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};
use crate::frac::*;
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
// -- Phase One --

pub fn serialize_ir(ir: &IntermediateRep) -> Result<Vec<u8>, FracFormatError> {
    let mut body_buf = Vec::new();
    for elm in &ir.body {
        write_doc_elm(&mut body_buf, elm)?;
    }

    let mut footnotes_buf = Vec::new();
    if let Some(footnotes) = &ir.footnotes {
        write_footnotes(&mut footnotes_buf, footnotes)?;
    }

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

// -- CORE DESERIALIZATION LOGIC --
// -- Phase Two --

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
    let mut footnotes = Vec::new();
    let num_footnotes = cursor.read_u64::<LittleEndian>()?;
    for _ in 0..num_footnotes {
        let title = read_string(&mut cursor)?;
        let num_body_elms = cursor.read_u64::<LittleEndian>()?;
        let mut note_body = Vec::new();
        for _ in 0..num_body_elms {
            note_body.push(read_doc_elm(&mut cursor)?);
        }
        footnotes.push(Footnote { title, body: note_body });
    }

    // --- Assemble IR ---
    // Note: Title, author etc. are not stored in the file format yet.
    Ok(IntermediateRep {
        body,
        footnotes: Some(footnotes),
        count: 0, // Can be recalculated if needed
        size: data.len() as u64,
        title: "".to_string(),
        author: None,
        last_modified: None,
        checksum: Some(format!("{:x}", calculated_checksum)),
        tags: None,
    })
}


// --- Serialization Helpers ---

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
        _ => { /* Unimplemented */ }
    }
    Ok(())
}

fn write_footnotes(w: &mut impl Write, footnotes: &[Footnote]) -> Result<(), FracFormatError> {
    w.write_u64::<LittleEndian>(footnotes.len() as u64)?;
    for note in footnotes {
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

// --- Deserialization Helpers ---

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
    Ok(Span { text, ..Span::new() })
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
