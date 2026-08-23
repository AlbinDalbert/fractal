# Fractal

Fractal is a small Rust library and CLI for projects containing native Fractal documents and hand-authored HTML.

Native documents use the `.fractal.html` suffix and a small semantic HTML profile. Other `.html` files are raw HTML: Fractal can inspect them, but their source belongs to the author. Both remain ordinary files that browsers and HTML tools can open.

## Principles

- **Native meaning, raw source.** Fractal may normalize native documents during semantic edits. It does not automatically rewrite raw HTML.
- **Files are the source of truth.** Fractal scans pages into memory when a project is opened. There are no persistent index or graph files to synchronize.
- **Links are explicit.** Fractal reads, validates, inserts, and preserves links. It never rewrites unlinked prose automatically.
- **Iframes are embeds.** Native documents may embed project files, inline `srcdoc`, or remote pages with ordinary `<iframe>` elements.
- **Suggestions are advisory.** Fractal can find link opportunities and return ranked candidate pages, including ambiguous candidates. Applying one is a separate explicit operation.
- **Derived data stays derived.** Search, links, backlinks, and suggestions are views over the current files.
- **The library is the product.** The CLI is a thin adapter over the Rust API.

Fractal does not try to make small models smart. It makes document operations cheap enough for any caller to compose.

## Project format

```text
my-project/
├── fractal.json
└── pages/
    ├── stockholm.fractal.html
    └── embeds/
        └── map.html
```

`fractal.json` contains only a project name and format version:

```json
{
  "name": "My project",
  "version": 1
}
```

A native document needs a safe relative `.fractal.html` path, a format marker, an identifiable title, and one document root. Its content uses standard semantic HTML. A raw `.html` file has no Fractal structure requirements.

See [`docs/format-contract.md`](docs/format-contract.md) for the complete contract.

## Core operations

The `Project` API provides:

- initialize and open projects;
- list, read, create, write, move, and delete pages;
- inspect explicit links and derived backlinks;
- inspect iframes and find pages that embed a project file;
- search page titles and visible text;
- suggest possible links without changing files;
- derive unambiguous exact-title links for runtime rendering;
- explicitly insert a selected link;
- validate titles and internal link targets.

Moving a page updates explicit internal links that target it. Single-file writes use atomic replacement.

## Link suggestions

Suppose a page contains "Stockholm" and the project contains:

- `stockholm.fractal.html`, titled "Stockholm";
- `stockholm-city.fractal.html`, titled "Stockholm City".

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
fractal --project <root> iframes <page>
fractal --project <root> backlinks <page>
fractal --project <root> embedded-by <page>
fractal --project <root> suggest <page>
fractal --project <root> derived-links <page>
fractal --project <root> link <page> <text> <target>
fractal --project <root> check
```

Every command supports `--json`. The default output is also deliberately simple and serializable while the CLI is young.

## Deliberate non-goals

The engine does not yet have:

- generated index or graph files;
- internal note primitive;
- automatic prose rewriting or sync command;
- embeddings, semantic search, entity extraction, or ontology;
- context-packet or token-budget machinery;
- import/export compiler framework;
- generalized transaction, command bus, or extension framework.

Import/export, richer queries, metadata, repair, and semantic tooling can be added as direct engine operations. UI policy and rich editing belong in applications that use the library.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Architecture is summarized in [`docs/architecture.md`](docs/architecture.md).
