# Fractal

Fractal is a small Rust library and CLI for projects containing native Fractal documents and hand-authored HTML.

Native documents use the `.fractal.html` suffix and a small semantic HTML profile. Other `.html` files are raw HTML: Fractal can inspect them, but their source belongs to the author. Both remain ordinary files that browsers and HTML tools can open.

## Principles

- **Native meaning, raw source.** Fractal may normalize native documents during semantic edits. It does not automatically rewrite raw HTML.
- **Files are the source of truth.** Fractal loads native documents into an in-memory catalog when a project is opened. It writes no generated catalog or link state.
- **Links are explicit.** Fractal reads, validates, inserts, and preserves links. It never rewrites unlinked prose automatically.
- **Iframes are embeds.** Native documents may embed project files, inline `srcdoc`, or remote pages with ordinary `<iframe>` elements.
- **Derived data stays derived.** Native text search, the native link index, backlinks, and exact-title derived links are in-memory views over the current files.
- **Hashes guard writes.** Every page exposes a SHA-256 source hash. Native content, managed CSS, user metadata, and head links also have independent hashes, so disjoint edits can merge without accepting stale changes to the same section.
- **The library is the product.** The CLI is a thin adapter over the Rust API.

Fractal does not try to make small models smart. It makes document operations cheap enough for any caller to compose.

## Project format

```text
my-project/
├── .fractal.lock
├── fractal.json
└── pages/
    ├── stockholm.fractal.html
    └── embeds/
        └── map.html
```

The empty `.fractal.lock` file coordinates Fractal processes; it is not document content. Older projects remain readable without it and acquire it on their first explicit mutation. The root `fractal.json` contains only a project name and format version:

```json
{
  "name": "My project",
  "version": 2
}
```

The manifest version identifies the Fractal project format, independently of the crate or CLI version. Format v1 is the stable contract recorded by the `contract-v1` Git tag. Format v2 is the active, unstable contract. New contract work remains part of v2 until a later `contract-v*` tag establishes another stable boundary.

A native document needs a safe relative `.fractal.html` path, a format marker, an identifiable title, and one document root. Its content uses standard semantic HTML. A raw `.html` file has no Fractal structure requirements.

Any folder below `pages/`, including `pages/` itself, may contain its own `fractal.json` with a title and an explicit order for direct child folders and native documents. Native filenames and nested directory names are kebab-case forms of their titles. Title changes rename them and update stored internal references. Read-only inspection reports path mismatches, and explicit project repair fixes them. Raw HTML paths remain independent. Without folder metadata, the title comes from the folder name and children use folder-first alphabetical order. The pages root uses the project name as its default title.

See [`docs/format-contract.md`](docs/format-contract.md) for the complete contract.

## Core operations

The `Project` API provides:

- initialize and open projects;
- inspect project health without writing files;
- explicitly recover interrupted transactions and apply format repairs;
- inspect folders, set folder titles, and explicitly order folder children;
- export an ordered folder tree as one HTML document;
- list, read, create, write, move, and delete pages;
- atomically recreate a missing native page from editor-owned recovery data;
- compare and write a page using its content hash;
- delete page batches or complete folders;
- inspect explicit links and derived backlinks;
- inspect iframes and find pages that embed a project file;
- search native document titles and visible native content;
- derive unambiguous exact-title links for runtime rendering;
- explicitly insert a selected link;
- validate titles and internal link targets.

Every mutation takes an exclusive lock on `.fractal.lock` and refreshes the in-memory native document catalog before checking paths or hashes. Raw source writes and native section writes compare the caller's hash while holding that lock. A mismatch returns `FractalErrorCode::Conflict`.

All project mutations use one recoverable transaction implementation. `MutationReceipt` reports created, updated, moved, and deleted project entries with direct path mappings and file hashes. Receipt paths are UTF-8, slash-separated, and relative to the project root.

`Project::open` and `Project::inspect` never repair or recover files. An interrupted transaction makes ordinary opening return `FractalErrorCode::RecoveryRequired`. `Project::recover` explicitly restores the pre-operation files and returns a `RecoveryReport`. `Project::repair` applies title-driven path and folder-order repairs and returns a `RepairReport`. Recovery and repair can contain several durable steps, so their reports retain completed changes and expose a typed `failures` list if a later step cannot finish.

A successful mutation has reached its durable commit point. If cleanup fails after commit, the mutation still returns its receipt with a `CleanupPending` warning. Inspection reports the leftover committed transaction until explicit recovery cleanup removes it.

## CLI

```text
fractal init <path> [--name <name>]
fractal --project <root> inspect
fractal --project <root> recover
fractal --project <root> list
fractal --project <root> folders
fractal --project <root> folder <folder>
fractal --project <root> set-folder-title <folder> <title>
fractal --project <root> set-page-title <page> <title>
fractal --project <root> move-folder <folder> <destination>
fractal --project <root> reorder-folder <folder> <child>...
fractal --project <root> read <page> [--source]
fractal --project <root> parts <page>
fractal --project <root> set-content <page> --file <html-file> --expected-hash <hash>
fractal --project <root> set-style <page> --file <css-file> --expected-hash <hash>
fractal --project <root> set-metadata <page> --file <html-file> --expected-hash <hash>
fractal --project <root> set-head-links <page> --file <html-file> --expected-hash <hash>
fractal --project <root> repair-page <page>
fractal --project <root> repair-project
fractal --project <root> new <title> [--path <path>]
fractal --project <root> recreate <page> --draft <draft-json>
fractal --project <root> write <page> --file <html-file> [--expected-hash <hash>]
fractal --project <root> move <page> <destination>
fractal --project <root> delete <page>
fractal --project <root> delete-pages <page>...
fractal --project <root> delete-folder <folder>
fractal --project <root> search <query>
fractal --project <root> links <page>
fractal --project <root> iframes <page>
fractal --project <root> backlinks <page>
fractal --project <root> embedded-by <page>
fractal --project <root> derived-links <page>
fractal --project <root> link <page> <text> <target>
fractal --project <root> export-html <page> --output <file> [--include-derived-links]
fractal --project <root> export-folder-html <folder> [selection]... --output <file> [--number-sections] [--include-derived-links] [--force]
fractal --project <root> check
```

Every command supports `--json`. The default output is also deliberately simple and serializable while the CLI is young.

`export-html` accepts one native document and writes one standalone HTML file. It keeps the source document's HTML content and inline styles, removes Fractal's native marker, replaces images and iframes with `[image]` and `[iframe]`, and drops external stylesheet links. Direct links to other native documents become links to one-level text references in a collapsed section at the bottom. Links to non-native project files are unwrapped without adding a reference. Derived native links can be included with `--include-derived-links`.

`export-folder-html` walks folders depth-first in their effective order and combines their native documents into one neutral HTML document. Each page receives a generated `<h1>` and an `<hr>` separates pages. Optional selections name relative pages or folders. A folder selection expands to all descendants unless more specific selections below it narrow the result. Included pages can be numbered from one. Ghosts are ignored. Invalid selected pages stop the export unless `--force` skips and reports them. Links between included pages become fragment links, while one-level references appear once at the end of the combined document. Optional derived links follow the same rule.

## Deliberate non-goals

The engine does not yet have:

- persistent generated catalog or link state;
- internal note primitive;
- automatic prose rewriting or sync command;
- embeddings, semantic search, entity extraction, or ontology;
- context-packet or token-budget machinery;
- import/export compiler framework;
- generalized transaction, command bus, or extension framework;
- persistent mutation history.

UI policy and rich editing belong in applications that use the library.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Architecture is summarized in [`docs/architecture.md`](docs/architecture.md).
