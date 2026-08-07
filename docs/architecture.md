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

`fractal.json` and files below `pages/` are the complete source of truth. `Project::open` scans HTML pages and derives titles, visible text, links, and backlinks in memory.

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
- Reload the in-memory catalog after mutations.
- Never combine link suggestion with link insertion.

Fractal does not promise database transactions. Multi-file move repair is deliberately straightforward; stronger machinery should be added only in response to demonstrated failure cases.

## Extension boundary

Notes, compilation, import/export, semantic search, embeddings, rich metadata, and model-specific context preparation are outside the core. Future implementations should first be ordinary consumers of the public API. They should enter the core only if they become necessary document operations rather than product-specific policy.
