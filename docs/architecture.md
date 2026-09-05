# Fractal Architecture

Fractal intentionally has four small layers:

```text
project files
    ↓
HTML parsing + in-memory native document catalog
    ↓
Native text search and native link operations
    ↓
Rust API / thin CLI
```

## Source of truth

The root `fractal.json`, folder `fractal.json` files, and content below `pages/` are the complete source of truth. `Project::open` scans folders, native documents, and raw HTML files without changing them. It builds an in-memory native document catalog that includes native titles and visible text, plus a native link index for stored links between native documents. Backlinks and exact-title derived links are computed from that state. Folder titles and explicit child orders remain stored user data.

Project format versions are contract versions, not crate releases. The `contract-v1` Git tag identifies the stable v1 contract. Current development uses the unstable v2 contract until another `contract-v*` tag establishes a stable boundary. Do not bump the project version for every compatible commit made within that development window.

Opening a project rebuilds the catalog and link index from project files. Fractal does not write persistent generated state or expose index lifecycle controls.

## Modules

- `project.rs`: project state and project module boundaries.
- `project/lifecycle.rs`: initialization, read-only opening, and explicit transaction recovery.
- `project/storage.rs`: native document and folder catalog loading, plus project locking.
- `project/page.rs` and `project/folder.rs`: page, folder, repair, and recreation operations.
- `project/links.rs`: native link index queries and explicit link insertion.
- `project/search.rs`: native text search and exact-title derived links.
- `project/validation.rs`: validation and read-only health inspection.
- `project/export.rs`: standalone page and folder HTML export.
- `project/support.rs`: path handling and the recoverable transaction implementation.
- `document.rs`: small parser-backed HTML extraction and mutation helpers.
- `types.rs`: serializable public values.
- `error.rs`: the compact public error type.
- `cli.rs`: a thin adapter over `Project`.

Prefer a direct function over a framework. Add a module only when one of these files has a distinct responsibility worth separating.

## Mutation rules

- Resolve and contain paths before accessing files.
- Validate the complete candidate native document after changing one owned section.
- Lock the stable `.fractal.lock` coordination file across each mutation and refresh the native document catalog after taking the lock. Legacy projects acquire this file on their first mutation without making ordinary opening or inspection write to disk.
- Hash exact page source bytes with SHA-256. Do not use modification times for conflict checks.
- Compare an expected hash and replace the page inside the same locked operation.
- Use the same recoverable transaction implementation for single-file and multi-file project mutations.
- Treat a page move and its backlink rewrites as one transaction.
- Update explicit folder orders in the same transaction as Fractal-managed child mutations.
- Preserve missing ordered children as ghosts until an explicit delete removes them.
- Report newly discovered children during inspection. Append them to an existing explicit order only during explicit repair.
- Stage folder and batch deletion as recoverable locked transactions.
- Update links and iframe sources only inside native documents.
- Never semantically rewrite raw HTML.
- Permit whole-source replacement only for raw HTML. Native callers edit content, managed CSS, user metadata, and head links through separate operations.
- Compare native section hashes under the project lock so disjoint concurrent edits merge without overwriting one another.
- Reload the in-memory native document catalog after mutations.
- Construct `MutationReceipt` from the committed change plan.
- Use project-root-relative UTF-8 report paths with `/` separators.

Folder HTML export is a derived read operation. It walks the in-memory folder catalog, validates selected native documents, rewrites links in temporary DOMs, and writes only the requested output file. It does not rewrite project documents or folder metadata.

Filesystem observers that ignore Fractal's project lock can see the individual renames in a disjoint batch or multi-file move. Fractal callers cannot. Inspection reports an interrupted transaction, ordinary opening refuses it, and `Project::recover` explicitly restores the pre-operation state.

`Ok` from a mutation means its commit marker reached durable storage. Cleanup trouble after that point becomes a receipt warning. Errors before commit trigger rollback. If rollback also fails, Fractal returns `FractalErrorCode::Indeterminate` and preserves the transaction directory for inspection.

## Application boundary

Fractal is an engine, not an editor. Rich-text controls, preview layout, and other UI policy belong in applications that use the crate.

Fractal owns stored content hashes, conditional writes, cross-process locking, page and folder deletion, page moves with backlink repair, mutation receipts, project inspection, and explicit transaction recovery. An editor owns unsaved revisions, durable draft storage, save timing, keystrokes received during a save, conflict dialogs, and recent receipt history.

Fractal exposes native text search, native link queries, and exact-title derived links as direct `Project` methods. It has no general graph traversal or query API.
