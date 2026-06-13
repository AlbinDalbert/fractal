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

## Phase 2 — Build a Safe Mutation Layer

### Problem

Most operations currently write files directly and then rebuild generated data. If an error occurs halfway through a rename, delete, sync, or metadata update, the project can be left partially changed.

### Goals

- Centralize file writes for project mutations.
- Make multi-file operations predictable.
- Return reliable reports for both applied and future dry-run operations.

### Work Items

- Introduce a mutation/write abstraction for project operations.
- Use temp files plus atomic rename for file writes where possible.
- Consider a project lock file during mutations.
- Separate mutation planning from mutation application.
- Make reports describe planned/applied semantic changes, not just raw file writes.
- Decide how much rollback support is required for failed multi-file operations.

### Candidate Operations to Move Onto This Layer

- page creation
- metadata updates
- note add/patch/remove
- safe editor page updates
- rename/move page
- delete page
- sync-generated links
- generated index/graph writes

### Acceptance Criteria

- A failed mutation does not leave generated data pretending everything succeeded.
- Broad operations like rename/delete/sync can report what they will change before writing.
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
- Add CLI `--json` / `--format json` eventually.
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
5. [ ] Introduce a mutation/write abstraction.
6. [ ] Move page/note/metadata operations onto the abstraction.
7. [ ] Normalize errors and reports.
8. [ ] Add JSON output and rustdoc for public surfaces.
9. [ ] Decide the page identity/path model.
10. [ ] Make generated data freshness explicit.
11. [ ] Then expand graph/search/import/export.

## Immediate Next Best Task

Finish the Phase 1 landing cleanup, then start Phase 2.

Before implementing the Phase 2 mutation layer, keep `README.md`, [`docs/format-contract.md`](docs/format-contract.md), validation, and tests aligned. The first Phase 2 design task is to introduce a small mutation/write abstraction and move one low-risk operation onto it before attempting broad rename/delete/sync rewrites.
