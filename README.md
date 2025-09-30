# Fractal
A minimalist text file format with some additional capabilities.

## Why?
Good question. simple answer, both Obsidian and Notion are about 85% of what I want, and I've not been able to find something that does and is exactly what I want. So I am making my own.

# Philosophy
It's oppinionated with the usage and simplicity as long as you use it as designed.
Seemless linking and isolation, shareable, exportable and importable.
Linking with other files is done on file name basis, not path. It's also automatic. 
Footnotes can be made when marking any word/string, this will *link* to a internal definition by default and can be exported to become external (it's own file).

## Link-By-Default
- Any mention of a file name (case-insensitive, normalized) becomes a link.
- You can exclude files from auto-linking via an ignore list or metadata flag.

## Internal Links / Footnotes
- Internal links are embedded directly in the file metadata.
- When exporting, all one-depth linked content is bundled inside the file.
- On import, internal links take precedence over external ones (solves name conflicts).
- You can convert a footnote to an external link with a click — seamless UX.

## UX Philosophy: Purposeful Simplicity
- No manual linking required — just write.
- No glossary management — the file system is the glossary.
- No broken links when sharing — internal links carry context.
- No clutter — footnotes are isolated, but convertible.

# tech
frontend: .NET avalonia (Amanite)
communication: MessagePack
backend: Rust

