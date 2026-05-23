# Fractal
A minimalist text file format with some additional capabilities.

## Why?
Good question. simple answer, both Obsidian and Notion are about 93% of what I want, and I've not been able to find something that does and is exactly what I want. So I am making my own.

# Philosophy
It's oppinionated with the usage and simplicity as long as you use it as designed.
Seemless linking and isolation, shareable, exportable and importable.
Linking with other files is done on file name basis, not path. It's also automatic. 
Footnotes can be made when marking any word/string, this will *link* to a internal definition by default and can be exported to become external (it's own file).

## Fractals
The fractal file is a project file which by default includes all .frac file below it in the file system. It's what manages external linking between the different notes and other metadata and wider stuff.

## Link-By-Default
- Any mention of a file name (case-insensitive, normalized) becomes a link.
- You can exclude files from auto-linking via an ignore list or metadata flag in the fractal.

## Internal Links / Footnotes
- Internal links are embedded directly in the file metadata.
- When exporting, all one-depth linked content is bundled inside the file.
- On import, internal links take precedence over external ones (solves name conflicts).
- You can convert a footnote to an external link (making it it's own .frac file).

## UX Philosophy: Purposeful Simplicity
- No manual linking required — just write.
- No broken links when sharing — internal links carry context. (external links are still broken unless explicitly stated).
- No clutter — footnotes are isolated, but convertible.

# Tech
backend: Rust

communication and formats:
- MessagePack
- JSON
- frac
- fractal
- markdown

# TODO

## Phase One: IR -> .frac Bytes

1. Serializing the DocElm Body
  This is the most complex part. You have a Vec<DocElm>, and DocElm is an enum. You need a clear strategy for writing this to disk.

   * Recommendation: Use a "tag-length-value" (TLV) or similar encoding scheme for each element in your Vec<DocElm>.
       * Tag: A single byte to identify the DocElm variant (e.g., 0x01 for Header, 0x02 for Paragraph).
       * Length: The size in bytes of the following value. This allows you to skip over elements you don't need to parse.
       * Value: The serialized data for that DocElm.

  For example, a Header element might be serialized as:
  [TAG_HEADER] [LENGTH] [LEVEL_BYTE] [SERIALIZED_SPAN]


## Phase Two: .frac Bytes -> IR 

## Phase Three: Markdown -> IR 
