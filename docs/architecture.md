# Fractal Architecture

Status: current working architecture, 2026-06-23. This is a small rulebook for keeping the engine boring, not a mandate to introduce heavyweight patterns.

## Core rule

Fractal owns the project/document truth. Callers may inspect raw files, but ordinary changes should go through the Rust API or CLI so the engine can preserve the Fractal format contract.

The normal operation shape is:

```text
resolve inputs
→ preflight invariants
→ plan file mutation
→ apply mutation through MutationPlan
→ rebuild generated data when needed
→ return OperationReport
```

If an operation does not fit this shape, document why in the operation code or its tests.

## Module boundaries

- `lib.rs` is the public crate surface. Keep exported functions/types intentional and editor-friendly.
- `types.rs` contains serializable public data shapes: manifest, index/graph records, editor DTOs, operation reports, and command-facing structs.
- `project/` owns project layout, manifest loading, path normalization, slug/path rules, and constants.
- `document/` owns HTML-backed Fractal page manipulation: rendering, metadata, notes, links, and parser-backed page edits.
- `validation.rs` owns the Fractal format contract. Mutation code may prepare candidate HTML, but validation decides whether it is valid Fractal.
- `index/` and `graph/` derive generated project data from page sources. Generated data is cache/output, not source of truth.
- `ops/` owns user-visible use cases: project/page/note/editor/sync/import/export operations. This is where modules are composed.
- `ops/mutation.rs` is the central write pipe for project mutations. It provides `MutationPlan`, atomic file replacement, and the project mutation lock.
- `cli.rs` is an adapter over the library API. The CLI should not become the core abstraction.

## Write rules

For user-visible project mutations:

1. Resolve paths through `project::paths` helpers.
2. Preflight collisions, label uniqueness, default-page impact, and other invariants before writing.
3. Build all candidate HTML/data in memory.
4. Validate candidate page HTML before it is written.
5. Apply filesystem changes through `MutationPlan` where practical.
6. Rebuild `.fractal/index.json` and `.fractal/graph.json` when source data changes.
7. Return an `OperationReport` with project-relative paths.

Direct filesystem calls are still fine for read-only operations, temporary local calculations, and narrowly scoped internals such as atomic write implementation. Direct writes in operations should be treated as suspicious unless there is a clear reason.

## Public API posture

The library API is first-class. Amanite, future editors, scripts, and agents should be able to use Fractal as a crate without shelling out to the CLI or hand-editing project files.

Prefer API functions that return structured data and typed reports over APIs that require callers to parse strings or rebuild Fractal invariants themselves.

## KISS pressure valves

- Do not add a generic service/container framework.
- Do not introduce traits until there are at least two real implementations or a test seam that clearly needs one.
- Prefer boring functions over object hierarchies.
- Prefer explicit operation functions over a universal command bus.
- Keep paths human-facing unless the page identity model deliberately changes.
- Keep generated data rebuildable.

## Current architecture risks

- Some roadmap text may lag behind the implementation; keep docs aligned after hardening work lands.
- Multi-step operations now use the mutation plan/lock pattern, but rollback and dry-run semantics are still intentionally limited.
- Error codes and machine-readable error context are still too coarse for mature editor/agent integration.
- Generated data freshness is partly visible through summaries but is not yet enforced consistently at every graph/search read.
