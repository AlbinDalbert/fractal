# Fractal architecture

Fractal is a library-first project engine for native Fractal documents. Its
runtime path is small:

```text
project files
    ↓
HTML parser and in-memory native document catalog
    ↓
native text search and native link index
    ↓
Rust API and CLI
```

## Source of truth

The root `fractal.json`, `.fractal.lock`, directories below `pages/`, native
`*.fractal.html` documents, and folder `fractal.json` metadata are the project
entries Fractal recognizes. Other files are opaque. They may coexist with
project entries, but Fractal does not list, read, validate, search, hash,
rewrite, export, or report them.

Project format 2 is the only supported format. `Project::open` rebuilds the
native document catalog and native link index from disk. It does not write
project files or persistent generated state. `Project::inspect`, validation,
search, and link derivation are also read-only.

The catalog contains every file with a `.fractal.html` suffix, including an
invalid native document. Validation then reports structural problems or broken
native links. Search uses native titles and visible native content. The link
index contains resolved native relationships and broken links that clearly
target missing native documents. Backlinks and exact-title derived links come
from the same native-only state.

Folder metadata stores a title and optional direct-child order. Fractal ignores
opaque files when it builds that order. Before a folder title change, move,
delete, or path repair, it scans the affected subtree. Opaque content makes the
operation fail before Fractal builds or commits a change plan.

## Modules

- `project.rs` contains project state and module boundaries.
- `project/lifecycle.rs` handles initialization, read-only opening, and explicit
  transaction recovery.
- `project/storage.rs` loads native documents and folders and manages locking.
- `project/page.rs` implements native document inspection and mutations.
- `project/folder.rs` implements folder creation, ordering, movement, deletion,
  and repair.
- `project/links.rs` handles native link queries and explicit insertion.
- `project/search.rs` handles native text search and exact-title derived links.
- `project/validation.rs` implements validation and read-only health inspection.
- `project/export.rs` writes standalone page and ordered-folder HTML exports.
- `project/support.rs` contains path checks and recoverable transactions.
- `document.rs` contains parser-backed HTML extraction and mutation helpers.
- `types.rs` contains serializable public values.
- `error.rs` contains the public error type.
- `cli.rs` maps CLI commands to `Project` methods.

## Mutation rules

- Resolve and contain paths before filesystem access.
- Require callers to create a folder before creating pages beneath it.
- Take the stable `.fractal.lock` across a mutation and reload the catalog after
  acquiring it.
- Compare section hashes while holding the lock.
- Validate the complete candidate native document after a section edit.
- Commit every normal project mutation with the common change plan and
  recoverable transaction code.
- Treat a page or folder move and all native backlink rewrites as one
  transaction.
- Update explicit folder order in the same transaction as a managed child
  creation, rename, move, or deletion.
- Preserve missing ordered children as ghosts until an explicit deletion.
- Refuse a folder title change, move, path repair, or deletion if its subtree
  contains opaque content.
- Reload the catalog after a successful mutation.
- Build `MutationReceipt` from the committed change plan.
- Use UTF-8 paths relative to the project root with `/` separators in reports.

An operation returns success after the transaction reaches its durable commit
point. A cleanup failure after that point becomes a `CleanupPending` warning in
the receipt. An error before commit rolls the operation back. If rollback also
fails, Fractal returns `FractalErrorCode::Indeterminate` and leaves the
transaction directory available for inspection and explicit recovery.

Filesystem observers that ignore `.fractal.lock` can see intermediate renames
during a multi-file operation. Fractal callers cannot. A pending interrupted
transaction blocks ordinary opening until `Project::recover` restores the old
state.

## Read and export boundaries

`Project::inspect` reports whether a project can open, pending recovery,
proposed title-driven repairs, validation results, and typed health issues. It
does not repair anything. `Project::repair` is an explicit mutation and records
completed changes and typed failures in its report.

Page and folder HTML exports are concrete output operations. They validate and
transform temporary DOMs, then write only the requested destination. They do
not mutate the project or imply a general conversion system.

## Application boundary

Fractal owns stored hashes, conditional native section writes, project locking,
page and folder mutations, receipts, inspection, validation, recovery, repair,
native search, native link queries, and HTML export.

Applications own unsaved revisions, durable draft storage, save timing,
conflict presentation, receipt history, and user-interface policy. Fractal is
not an end-user editor.
