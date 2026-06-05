# Fractal Design Brief for Clean-Slate CLI Discussion

This document describes what Fractal is, what the engine is responsible for, and what behavior the tool must preserve. It intentionally does not describe the current command-line interface, command names, flag names, argument order, output format, or implementation details. Use it as context when asking for fresh interface design ideas.

## What Fractal Is

Fractal is an engine for durable, file-backed knowledge projects.

Its purpose is to help people create, maintain, validate, repair, search, link, query, import, and export structured knowledge collections made of linked pages. It is not primarily a markdown notes app, a generic website generator, or an end-user graphical editor.

The long-term shape is closer to a reference engine for a document/project format: Fractal owns the project contract, the page conventions, the generated indexes, and the graph behavior. Other tools, including editors and LLM workflows, should be able to build on top of it.

## Product Direction

Fractal is meant to provide better infrastructure for linked pages than markdown-oriented personal knowledge tools.

The user-facing files should stay ordinary enough to inspect, edit, and render with common HTML tools. If Fractal disappears, the pages should still be readable and mostly usable in a normal browser. At the same time, Fractal documents are stricter than arbitrary HTML. HTML is the storage layer, but valid Fractal content follows Fractal's own rules.

Fractal should reduce the amount of graph maintenance users have to do manually. Users should not need to remember every page that should be linked, every backlink that should exist, or every structural relationship that might matter later. The engine should increasingly infer, maintain, validate, and expose that structure.

Fractal should also be useful to LLMs, including small local models. The engine should offload work such as graph search, semantic lookup, indexing, summaries, relationship discovery, and context extraction, so an LLM does not need to spend large token budgets rediscovering the shape of a knowledge base.

## Core Engine Responsibilities

Fractal should provide reliable project operations:

- Create a valid project.
- Validate whether a project still follows the Fractal contract.
- Repair missing or damaged Fractal-owned structure when that can be done safely.
- Keep generated project data up to date.
- Report what changed during mutations.
- Preserve user-authored content wherever possible.

Fractal should provide reliable document operations:

- Create pages.
- Read page content and metadata.
- Mutate Fractal-owned parts of a page safely.
- Rename or move pages while preserving project invariants.
- Delete pages while reporting affected relationships.
- Preserve valid Fractal structure during edits.

Fractal should provide discovery and graph operations:

- Index pages, metadata, links, notes, and relationships.
- Track backlinks and outlinks.
- Query page relationships.
- Detect orphaned pages or weakly connected areas.
- Support search across titles, summaries, tags, note labels, link text, and page content.
- Grow toward semantic search, entity extraction, aliases, relationship types, concept typing, summaries, and embeddings.

Fractal should provide import and export operations:

- Convert external formats into valid Fractal pages.
- Convert Fractal content back out to other useful formats.
- Treat portability as part of the core contract, not as a side feature.

## Format Principles

Fractal pages are HTML-backed. This gives users durable, inspectable files and gives Fractal a richer rendering target than markdown.

Valid Fractal is stricter than valid HTML. Fractal may require specific project structure, page metadata, page markers, generated sections, or managed attributes. Those requirements are part of the document contract.

The engine should use robust HTML parsing for extraction, validation, mutation, link rewriting, import, and export. It should not depend on fragile string assumptions such as exact indentation, attribute order, or quote style.

Fractal-owned generated data should be clearly distinguishable from user-authored content. Generated links and generated project data should be replaceable without damaging manual edits.

## Linking Model

Links are project structure, not just hand-authored markup.

Fractal should infer links only when the target is known and unambiguous. Page labels are derived from page titles and filename-like identifiers. Matching is normalized and case-insensitive. Duplicate labels are invalid project state for now, even if the pages live in different folders.

Unknown prose should remain ordinary text. Fractal should not treat every unmatched phrase as a broken link or unresolved reference.

Fractal should support page-local notes. Note trigger text can be linked within the page where the note lives. Page-to-page links and page-to-note links are both part of the graph.

Rerunning link generation should update Fractal-managed links without rewriting unrelated manual links.

## Metadata Model

Pages should support Fractal-owned metadata such as:

- Title.
- Summary.
- Tags.
- Format version.
- Notes.
- Links.
- Backlinks.
- Outlinks.

The metadata model should grow over time to support richer summaries, editable tags, stable page identifiers, schema migrations, aliases, entities, relationship types, and semantic indexes.

## Graph and Index Model

Fractal maintains generated project data so tools do not need to repeatedly rescan every page for common operations.

The generated index should summarize page paths, titles, metadata, notes, and links.

The generated graph should represent pages, notes, and relationships between them. It should expose both outgoing links and backlinks. It should be versioned so unsupported graph data is rejected rather than guessed.

The graph is a central engine feature. Linking, discovery, traversal, search, LLM lookup, orphan detection, related-page suggestions, and semantic tooling should build on it.

## Editor and API Direction

The Rust library API is a first-class product surface. A future editor should be able to use Fractal directly as a crate rather than shelling out to a command-line wrapper or hand-editing project files.

Safe library mutations should preserve Fractal-owned structure, rebuild generated data when appropriate, and return structured reports describing what changed.

Raw source access can exist as an escape hatch, but it should not be the only practical way to build an editor. Higher-level operations should exist for normal page, metadata, note, link, rename, move, delete, validate, repair, import, export, search, and graph workflows.

UI-specific concerns should stay outside the engine crate.

## LLM Workflow Direction

Fractal should help LLMs work with large knowledge bases without reading everything.

Useful LLM-oriented capabilities include:

- Compact context extraction from pages.
- Search that can return small, relevant snippets.
- Graph queries that reveal local neighborhoods and related concepts.
- Summaries and tags that help choose what to read next.
- Semantic lookup through embeddings or equivalent indexes.
- Entity and relationship extraction.
- Stable, parseable operation reports.
- Low-token representations of project structure.

Fractal should make it easy for an LLM or agent to ask the engine targeted questions about the knowledge base instead of reconstructing the graph from raw files.

## Design Constraints for Interface Proposals

When proposing a command-line interface, API shape, or agent-facing interface for Fractal, assume the interface is a clean slate.

Do not infer any existing command names, command hierarchy, flags, aliases, output formats, argument order, or help text from this document. Those choices are intentionally omitted.

The interface should still respect these product constraints:

- Fractal is an engine for the Fractal project/document format.
- The CLI should expose engine capabilities without turning the crate into an end-user UI application.
- The Rust library API should remain coherent and first-class.
- The interface should make project, page, graph, search, import, export, validation, repair, indexing, linking, and mutation workflows understandable.
- Human users and LLM agents should both be able to operate it predictably.
- Output should be suitable for both humans and automation where possible.
- Destructive or broad mutations should be explicit and report what changed.
- The interface should make invalid project state clear instead of silently guessing.

## Open Design Questions

These are intentionally left open for clean-slate design discussion:

- What should the command hierarchy be?
- Which operations deserve top-level commands?
- Which operations should be grouped under resources such as projects, pages, graphs, links, notes, search, import, or export?
- What should default output look like?
- When should structured machine-readable output be available?
- How should paths and page identifiers be accepted?
- How should validation, repair, indexing, and link generation relate to ordinary edits?
- How should agents discover capabilities from help text or schemas?
- How should the interface balance concise human ergonomics with predictable automation?
