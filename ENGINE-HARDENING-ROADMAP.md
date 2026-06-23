# Fractal Engine Hardening Roadmap

This roadmap captures the next engineering push for Fractal after the current pre-alpha engine work. The goal is to move from a working prototype into a durable document/project engine that can safely support a future editor, richer graph/search behavior, and LLM-oriented workflows.

The current codebase has good bones. The risk is not that the implementation is fundamentally bad; the risk is that important format and mutation invariants are still implied by README text, tests, and convention rather than enforced through a central contract.

## Guiding Principles

- Keep Fractal an engine for the Fractal project/document format, not an end-user UI app.
- Keep the Rust library API first-class; the CLI should wrap engine operations, not define the core abstraction.
- Preserve human-readable, inspectable, browser-renderable HTML files.
- Treat paths as an important part of the user-facing design.
- Make generated data, mutation reports, and validation errors reliable enough for humans, editors, scripts, and LLM agents.
- Prefer explicit invalid-state reporting over silent guessing.

## Phase 1 — Define and Enforce the Fractal Page Contract

Status: landed as the current baseline. The short-form contract now lives in [`docs/format-contract.md`](docs/format-contract.md); this section records the design decisions behind it.

### Problem

Fractal says that valid Fractal documents are stricter than arbitrary HTML, but validation currently checks only part of that contract. Many assumptions exist in mutation/indexing code without being enforced centrally.

### Goals

- Define what a valid Fractal page must contain.
- Make validation the source of truth for the page contract.
- Keep `validate --fix` limited to safe repairs.

### Landed Work

- Added strict project/page validation for the Phase 1 contract.
- Added safe `validate --fix` repairs for approved scaffold, stylesheet, title/theme/meta/notes, and simple manual-link drift.
- Added validation before raw source writes through `write_page_source`.
- Added tests for strict metadata, title/heading consistency, body element restrictions, generated-link resolution, repairable drift, and source-write rejection.
- Added [`docs/format-contract.md`](docs/format-contract.md) as the current compact contract reference.

### Phase 1 Contract Decisions

- Required document structure:
  - generated source includes `<!doctype html>`, `<html>`, `<head>`, and `<body>`
  - validation is parser-backed and currently requires one effective `<head>` and `<body>` in the parsed document tree rather than literal source-string checks for those tags
  - exactly one `<main>`
  - exactly one notes section: `<section data-fractal-notes>`
  - the notes section must be a direct child of `<body>`
  - the notes section must live outside `<main>`
  - notes outside the notes section are invalid
- Title structure:
  - every page must have both `<title>` and a page heading in `<main>`
  - the page heading must be an `<h1>`
  - the `<h1>` must be the first meaningful child of `<main>`
  - `<title>` and `<main h1>` must match after normalization
  - the normalized title must be non-empty
- Allowed page and note body structure:
  - Fractal is not treating arbitrary manual HTML as valid input at this stage
  - all normal changes should happen through the crate API or CLI
  - after the required page `<h1>`, `<main>` may contain only this subset for now:
    - `p`
    - `h2` through `h6`
    - `ul`, `ol`, `li`
    - `blockquote`
    - `pre`, `code`
    - generated links: `a[data-fractal-link]`
  - note content follows the same structural allowance as the main body
- Required Fractal metadata:
  - exactly one `fractal:version`
  - required supported version
  - exactly one `fractal:summary`
  - exactly one `fractal:tags`
  - summary and tags may be empty
  - extra `fractal:*` meta tags are not allowed for now
  - ordinary non-Fractal meta tags are allowed
- Required body/project markers:
  - stylesheet link to `.fractal/style.css`
  - stylesheet href must match the exact computed relative path for the page depth
  - `validate --fix` may overwrite/restore the default stylesheet link if it is missing or wrong
  - `<body data-fractal-theme>` must match the manifest theme
  - only `dark` and `light` are valid themes for now
- Notes rules:
  - note IDs must be valid
  - note IDs must be unique per page
  - each note must be an `<aside id="note-*" data-fractal-note>` inside the notes section
  - note body content follows the same allowed element subset as page body content
- Link rules:
  - only Fractal-generated links are supported for now
  - generated links must use `data-fractal-link`
  - `data-fractal-link="page"` links must resolve to a known page
  - `data-fractal-link="note"` links must resolve to a known note in the same page
  - manual `<a href="...">` links are invalid during normal validation
  - `validate --fix` may unwrap simple manual links into plain text while preserving their text content
- Raw source writes:
  - `write_page_source` must validate the new HTML before saving
  - invalid Fractal HTML should be rejected instead of written

### Acceptance Criteria

- Validation catches every structural assumption used by indexing, graphing, sync, and editor operations.
- Tests distinguish:
  - valid Fractal HTML,
  - repairable Fractal-like HTML,
  - arbitrary HTML that is not valid Fractal.
- README and code agree on the contract.

## Phase 2 — Finish the Safe Mutation Layer

Status: partially landed. The code now has the baseline abstraction (`MutationPlan`), atomic file replacement, a project mutation lock, typed operation events, and preflight helpers for the riskiest page operations. The remaining work is consistency, failure semantics, and documentation—not inventing a larger architecture.

### Problem

Most important operations now use a central mutation path, but the project still needs a deliberate audit of every write path. Multi-file operations are safer than before, but they are not fully transactional: a late failure can still leave earlier applied changes on disk. The engine should be honest about those limits and avoid reporting generated data as fresh when an operation did not fully complete.

### Goals

- Keep project writes centralized through `MutationPlan` where practical.
- Make multi-file operations predictable and explicitly non-magical.
- Keep preflight/report behavior stable enough for editors, scripts, and agents.
- Define the limits of locking, dry-run/preflight, and rollback.

### Landed Work

- Added `MutationPlan` as the central mutation/write abstraction.
- Added atomic replacement for file writes through temporary files and rename.
- Added a project mutation lock under `.fractal/mutation.lock` while plans apply.
- Added typed `OperationEvent` / `OperationReport` summaries with project-relative paths.
- Planned link rewrites before destructive rename/delete operations.
- Added preflight helpers for rename and delete impact reporting.
- Added tests around mutation locking, operation summaries, rename/delete preflights, and generated-data reporting.

### Remaining Work Items

- Audit direct filesystem writes in operation modules and either move them onto `MutationPlan` or document why they are not project mutations.
- Keep page creation, metadata updates, note add/patch/remove, editor updates, rename/move, delete, sync, and generated index/graph writes on the same mutation/report conventions.
- Decide whether preflight structs are enough for dry-run UX or whether selected operations need first-class dry-run reports.
- Decide how much rollback support is required for failed multi-file operations.
- Add failure-path tests for partially failing rename/delete/sync-style operations where practical.
- Keep reports semantic: callers should understand user-content changes, source-file changes, generated-data changes, and no-ops without parsing strings.

### Acceptance Criteria

- User-visible project mutations either use `MutationPlan` or have a documented reason not to.
- A failed mutation does not leave generated data pretending everything succeeded.
- Broad operations like rename/delete/sync can report their expected impact before destructive writes where that matters.
- Every write operation has a consistent report shape.

## Phase 3 — Stabilize Errors, Reports, and Machine Output

### Problem

The public API has `FractalErrorCode`, but many errors collapse into generic string-based `InvalidInput`. CLI errors are also not currently friendly for either humans or automation.

### Goals

- Make errors stable and specific.
- Make mutation reports stable and parseable.
- Prepare the CLI/API for editor and LLM-agent use.

### Work Items

- Expand error codes into meaningful categories, such as:
  - project not found
  - manifest missing/invalid
  - page not found
  - page already exists
  - invalid Fractal page
  - unsupported manifest/page/index/graph version
  - duplicate page label
  - duplicate note ID
  - stale index/graph
  - path outside project
- Add structured error context where useful:
  - paths
  - page labels
  - expected version
  - actual version
  - offending note/link ID
- Version the operation report schema.
- Normalize report paths to project-relative paths where possible.
- Finish normalizing CLI JSON coverage for migrated commands and keep the output envelope stable.
- Add rustdoc for the public library API.

### Acceptance Criteria

- Humans get concise CLI errors.
- Agents/editors can branch on stable error codes.
- Write operations produce reliable machine-readable reports.

## Phase 4 — Clarify Page Identity and Path Semantics

### Problem

Current graph node identity is path-based, e.g. `page:index.html`. This is simple and user-visible, and paths are an important part of Fractal's design. But path-only identity may become limiting for rename history, genealogy, semantic overlays, external references, and long-lived editor state.

This phase should not blindly replace paths with opaque IDs. The design needs to preserve path-first ergonomics while deciding whether the engine needs a more stable internal identity layer.

### Current Position

Paths are valuable because they are:

- human-readable,
- inspectable in the filesystem,
- easy to reason about,
- compatible with ordinary HTML links,
- aligned with Fractal's file-backed design.

So paths should remain first-class and probably remain the primary user-facing address for pages.

### Questions to Answer

- Is a page's path its identity, or its current address?
- What should happen to identity across rename/move?
- Do backlinks/history need to survive path changes as the same page?
- Should external tools refer to pages by path, stable ID, or either?
- Should graph node IDs remain path-derived?
- Can Fractal support path-first UX while still storing stable internal IDs?

### Candidate Models

#### Option A — Path-as-Identity

Keep the current model. A page is identified by its path.

Pros:

- Simple.
- Transparent.
- Matches ordinary files and HTML.
- No hidden metadata requirement.

Cons:

- Rename/move changes identity.
- Harder to track genealogy.
- External references can break unless rewritten everywhere.
- Semantic indexes may need full remapping after moves.

#### Option B — Path-as-Identity Plus Genealogy

Keep path identity, but record move/rename events in project history.

Pros:

- Preserves path-first design.
- Gives future tooling some continuity.
- Avoids mandatory opaque IDs in pages.

Cons:

- History becomes important for resolving old references.
- Current page identity is still path-based.
- Harder for long-lived external integrations.

#### Option C — Stable Internal Page ID, Path as Canonical Address

Store a stable page ID in Fractal metadata, but keep paths as the normal interface.

Pros:

- Renames preserve identity.
- Better for graph history, semantic indexes, editor state, and external integrations.
- Paths stay human-facing.

Cons:

- Adds hidden machinery.
- Requires ID generation, migration, validation, and repair.
- Can feel less file-native if exposed poorly.

### Recommended Approach For Now

Do not rush this. Rename the problem from “replace paths with IDs” to “define Fractal page identity.”

Near-term:

- Keep paths as the primary public interface.
- Make path behavior explicit in validation, graph, reports, and docs.
- Add tests around rename/move identity expectations.

Later spike:

- Prototype stable internal IDs behind the path-first API.
- Evaluate whether IDs reduce real complexity in graph/search/editor workflows.
- Only adopt IDs if they clearly improve long-term engine behavior without undermining Fractal's file-backed design.

### Acceptance Criteria

- The project has a written identity model.
- Paths remain first-class.
- Any future ID system is internal/supporting unless there is a strong reason to expose it.

## Phase 5 — Make Generated Data Freshness Explicit

### Problem

Some operations build live state from pages, while others load `.fractal/index.json` or `.fractal/graph.json`. This makes it possible to query stale generated data without realizing it.

### Goals

- Make index/graph freshness a clear contract.
- Avoid silent stale reads.
- Let editors and agents know when generated data is current.

### Work Items

- Define freshness rules for index and graph reads.
- Decide whether query APIs should:
  - refuse stale generated data,
  - auto-rebuild stale generated data,
  - or return explicit `stale: true` status.
- Store enough generated-data metadata to compare source state cheaply.
- Avoid silently swallowing page read errors during search.
- Make `project_summary` style freshness checks reusable.

### Acceptance Criteria

- Graph/search queries never unknowingly return stale data.
- Generated files are clearly treated as rebuildable cache, not silent source of truth.
- Editors know when to call sync/build/validate.

## Phase 6 — Grow Graph, Search, Import, and Export on the Hardened Core

### Problem

Graph/search/import/export are core to Fractal's long-term direction, but expanding them before the format and mutation layers are hardened will compound debt.

### Goals

- Build richer capabilities only after the core invariants are reliable.
- Keep graph/search useful to humans and LLM agents.
- Keep import/export part of the core portability story.

### Future Work Areas

- Richer graph edges:
  - aliases
  - entity mentions
  - typed relationships
  - inferred relationships
  - broken/manual/generated link distinctions
- Better search:
  - snippets
  - field filters
  - tag filters
  - ranking
  - semantic search / embeddings
- Better sync/linking:
  - aliases
  - Unicode-aware matching
  - link density controls
  - faster multi-pattern matching
- Better import/export:
  - richer Markdown support
  - HTML import into valid Fractal pages
  - JSON or JSONL export for agents
  - graph export formats
- LLM-oriented context APIs:
  - compact page context
  - graph neighborhoods
  - token/character budgets
  - low-noise summaries/snippets

### Acceptance Criteria

- New graph/search/import/export features build on validated page/project contracts.
- Generated data remains versioned and freshness-aware.
- Public APIs stay coherent and editor-friendly.

## Suggested Execution Order

1. [x] Write the Fractal page/project contract as a short spec.
2. [x] Update validation to enforce the spec.
3. [x] Update repair to safely fix only agreed scaffold problems.
4. [x] Add missing tests around invalid structure and edge cases.
5. [x] Introduce a mutation/write abstraction.
6. [ ] Audit remaining write paths and finish moving project mutations onto the abstraction.
7. [ ] Normalize errors and reports.
8. [ ] Finish JSON output normalization and add rustdoc for public surfaces.
9. [ ] Decide the page identity/path model.
10. [ ] Make generated data freshness explicit.
11. [ ] Then expand graph/search/import/export.

## Immediate Next Best Task

Do a Phase 2 consistency pass rather than a new architecture pass.

Specifically: follow [`docs/current-focus.md`](docs/current-focus.md), audit direct filesystem writes, document any intentional exceptions, add missing failure-path tests, and keep `README.md`, [`docs/architecture.md`](docs/architecture.md), [`docs/format-contract.md`](docs/format-contract.md), validation, and tests aligned with what the mutation layer actually guarantees.
