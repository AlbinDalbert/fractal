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

`fractal.json` and files below `pages/` are the complete source of truth. `Project::open` scans native and raw HTML files and derives titles, visible text, links, backlinks, and iframe references in memory.

Do not add persistent generated state until measured project sizes prove scanning inadequate. If a cache is eventually needed, it must remain disposable and invisible to the format contract.

## Modules

- `project.rs`: project loading, catalog construction, operations, search, backlinks, validation, and suggestions.
- `document.rs`: small parser-backed HTML extraction and mutation helpers.
- `types.rs`: serializable public values.
- `error.rs`: the compact public error type.
- `cli.rs`: a thin adapter over `Project`.

Prefer a direct function over a framework. Add a module only when one of these files has a distinct responsibility worth separating.

## Mutation rules

- Resolve and contain paths before accessing files.
- Validate candidate page source before replacing an existing page.
- Use atomic replacement for single-file writes.
- Update explicit backlinks when moving a target page.
- Update links and iframe sources only inside native documents.
- Never semantically rewrite raw HTML.
- Reload the in-memory catalog after mutations.
- Never combine link suggestion with link insertion.

Fractal does not promise database transactions. Multi-file move repair is deliberately straightforward; stronger machinery should be added only in response to demonstrated failure cases.

## Application boundary

Fractal is an engine, not an editor. Rich-text controls, preview layout, and other UI policy belong in applications that use the crate.

Import/export, repair, indexing, graph queries, metadata, summaries, and semantic tooling belong in the engine when they become concrete project or document operations. Add them directly before introducing a general framework.
