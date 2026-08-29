# Fractal Architecture

Fractal intentionally has four small layers:

```text
project files
    ↓
HTML parsing + in-memory catalog
    ↓
Project operations and derived queries
    ↓
Rust API / thin CLI
```

## Source of truth

The root `fractal.json`, folder `fractal.json` files, and content below `pages/` are the complete source of truth. `Project::open` scans folders, native documents, and raw HTML files. It derives document titles, visible text, links, backlinks, and iframe references in memory. Folder titles and explicit child orders are stored user data, not generated indexes.

Project format versions are contract versions, not crate releases. The `contract-v1` Git tag identifies the stable v1 contract. Current development uses the unstable v2 contract until another `contract-v*` tag establishes a stable boundary. Do not bump the project version for every compatible commit made within that development window.

Do not add persistent generated state until measured project sizes prove scanning inadequate. If a cache is eventually needed, it must remain disposable and invisible to the format contract.

## Modules

- `project.rs`: project loading, catalog construction, operations, search, backlinks, validation, and derived links.
- `document.rs`: small parser-backed HTML extraction and mutation helpers.
- `types.rs`: serializable public values.
- `error.rs`: the compact public error type.
- `cli.rs`: a thin adapter over `Project`.

Prefer a direct function over a framework. Add a module only when one of these files has a distinct responsibility worth separating.

## Mutation rules

- Resolve and contain paths before accessing files.
- Validate candidate page source before replacing an existing page.
- Lock `fractal.json` across each mutation and refresh the catalog after taking the lock.
- Hash exact page source bytes with SHA-256. Do not use modification times for conflict checks.
- Compare an expected hash and replace the page inside the same locked operation.
- Use atomic replacement for single-file writes.
- Treat a page move and its backlink rewrites as one recoverable file transaction.
- Update explicit folder orders in the same transaction as Fractal-managed child mutations.
- Preserve missing ordered children as ghosts until an explicit delete removes them.
- Append newly discovered children to an existing explicit order.
- Commit folder deletion with one same-filesystem rename. Stage batch deletion as a recoverable locked transaction.
- Update links and iframe sources only inside native documents.
- Never semantically rewrite raw HTML.
- Reload the in-memory catalog after mutations.

Filesystem observers that ignore Fractal's project lock can see the individual renames in a disjoint batch or multi-file move. Fractal callers cannot. An interrupted transaction is rolled back when the next process opens or mutates the project.

## Application boundary

Fractal is an engine, not an editor. Rich-text controls, preview layout, and other UI policy belong in applications that use the crate.

Fractal owns stored content hashes, conditional writes, cross-process locking, page and folder deletion, and page moves with backlink repair. An editor owns unsaved revisions, save timing, keystrokes received during a save, and conflict dialogs.

Import/export, repair, indexing, graph queries, metadata, summaries, and semantic tooling belong in the engine when they become concrete project or document operations. Add them directly before introducing a general framework.
