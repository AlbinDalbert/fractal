# Fractal

Fractal is a small Rust library and CLI that makes operations on linked HTML documents cheap, reliable, and composable.

It manages portable HTML pages without turning them into a database or a private rendering format. Pages remain ordinary files that browsers, editors, scripts, people, and models can use directly.

## Principles

- **HTML is the document format.** Ordinary HTML, manual links, external links, and normal semantic elements are allowed.
- **Files are the source of truth.** Fractal scans pages into memory when a project is opened. There are no persistent index or graph files to synchronize.
- **Links are explicit.** Fractal reads, validates, inserts, and preserves links. It never rewrites unlinked prose automatically.
- **Suggestions are advisory.** Fractal can find link opportunities and return ranked candidate pages, including ambiguous candidates. Applying one is a separate explicit operation.
- **Derived data stays derived.** Search, links, backlinks, and suggestions are views over the current files.
- **The library is the product.** The CLI is a thin adapter over the Rust API.

Fractal does not try to make small models smart. It makes document operations cheap enough for any caller to compose.

## Project format

```text
my-project/
├── fractal.json
└── pages/
    ├── stockholm.html
    └── travel/
        └── sweden.html
```

`fractal.json` contains only a project name and format version:

```json
{
  "name": "My project",
  "version": 1
}
```

A page needs a safe relative `.html` path and an identifiable title from `<title>` or `<h1>`. Fractal does not require special metadata, stylesheets, generated sections, link attributes, themes, or notes.

See [`docs/format-contract.md`](docs/format-contract.md) for the complete contract.

## Core operations

The `Project` API provides:

- initialize and open projects;
- list, read, create, write, move, and delete pages;
- inspect explicit links and derived backlinks;
- search page titles and visible text;
- suggest possible links without changing files;
- explicitly insert a selected link;
- validate titles and internal link targets.

Moving a page updates explicit internal links that target it. Single-file writes use atomic replacement.

## Link suggestions

Suppose a page contains “Stockholm” and the project contains:

- `stockholm.html`, titled “Stockholm”;
- `stockholm-city.html`, titled “Stockholm City”.

Fractal groups both under the same suggestion. The exact title match ranks first and the partial token match remains available as an alternative. Existing explicit links are not suggested again.

Initial ranking is intentionally understandable:

1. exact title;
2. exact filename stem;
3. title-token overlap.

Suggestions never mutate source. `insert_link` is a separate operation that links a caller-selected occurrence to a caller-selected target.

## CLI

```text
fractal init <path> [--name <name>]
fractal --project <root> list
fractal --project <root> read <page> [--source]
fractal --project <root> new <title> [--path <path>]
fractal --project <root> write <page> --file <html-file>
fractal --project <root> move <page> <destination>
fractal --project <root> delete <page>
fractal --project <root> search <query>
fractal --project <root> links <page>
fractal --project <root> backlinks <page>
fractal --project <root> suggest <page>
fractal --project <root> link <page> <text> <target>
fractal --project <root> check
```

Every command supports `--json`. The default output is also deliberately simple and serializable while the CLI is young.

## Deliberate non-goals

The core currently has no:

- generated index or graph files;
- internal note primitive;
- automatic prose rewriting or sync command;
- embeddings, semantic search, entity extraction, or ontology;
- context-packet or token-budget machinery;
- mandatory metadata, theme, stylesheet, or restricted HTML subset;
- import/export compiler framework;
- generalized transaction, command bus, or extension framework.

If actual usage demonstrates a need, these can be built later without making them part of the foundational document contract.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Architecture is summarized in [`docs/architecture.md`](docs/architecture.md).
