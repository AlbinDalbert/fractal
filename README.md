# Fractal

`fractal` is a reset-in-progress Rust engine and CLI for a file-backed knowledge base / graph format.

This repo is currently focused on one thing: locking down the first useful crate API, format contract, and project layout before deeper graph behavior and CLI ergonomics are rebuilt.

## Project direction

Fractal is meant to be better tooling, engine infrastructure, and user experience for linked pages than markdown-oriented knowledge tools such as Obsidian.

Fractal is the engine, not an end-user interface. This crate should focus on the durable file format, project layout, indexing, graph construction, linking, metadata, import/export, search, and programmatic command surface that other tools can build on. A separate UI can consume Fractal data later, but this project should not take on interactive application UI concerns.

The page format is HTML-backed by design, but Fractal is not arbitrary HTML. HTML is the storage/rendering substrate in roughly the same way DOCX borrows XML: inspectable with common tools, but governed by Fractal's stricter format contract. Fractal avoids owning a custom renderer while still getting a richer and more consistent rendering target than markdown.

The current Fractal format contract is documented in [`docs/format-contract.md`](docs/format-contract.md). Treat that file as the short-form contract for what validation is expected to enforce.

The core product idea is that users should not have to remember which pages to link to, when to link them, or how to maintain the graph by hand. Linking, discovery, metadata, indexing, and graph structure should increasingly be inferred and maintained by the tool.

There is a second-order goal behind the same design: Fractal should become a human-usable knowledge system that is also useful to LLMs, including small local models. The engine should offload graph search, semantic lookup, indexing, summaries, and other ontological work from the model, so an LLM does not need to spend large token budgets rediscovering the structure of a knowledge base. In that sense Fractal is a slim RAG / knowledge-graph engine whose data model is shared with the user-facing linked-page tool.

Because of that, import and export are not side features. Converting other formats into Fractal, and converting Fractal pages back out again, is part of the long-term contract: the project should make knowledge portable while still giving Fractal enough structure to build fast, semantic, programmatic tooling on top.

## Core next tasks

Near-term priority is engine hardening before broad feature growth. See [`ENGINE-HARDENING-ROADMAP.md`](ENGINE-HARDENING-ROADMAP.md) for the phase plan.

- Land the Phase 1 baseline: keep [`docs/format-contract.md`](docs/format-contract.md), this README, validation, and tests aligned.
- Build the Phase 2 safe mutation layer: centralize project writes, preflight multi-file operations, use atomic writes where practical, and make failure behavior predictable.
- Stabilize errors and reports so humans, editors, scripts, and LLM agents can branch on stable machine-readable outcomes instead of parsing strings.
- Make generated index/graph freshness explicit for graph and search reads.
- Then grow graph/search/import/export on the hardened core: richer edge types, snippets, field filters, semantic search, aliases/entities/relationships, context packets, and richer import/export.

## Pre-alpha editor todo

These are the engine-facing tasks that should be done before starting a basic editor in earnest. The goal is not to build UI in this crate, but to give a future Tauri editor or other application a stable enough Rust API that it can use Fractal directly instead of shelling out to the CLI or hand-editing project files.

- [x] Page list/detail API: add a structured way to list project pages and inspect one page's editor-relevant data. A page list should include path, title, summary, tags, and small graph facts such as backlink and outlink counts. A page detail call should return the page source plus metadata, notes, links, backlinks, and outlinks. This gives an editor enough data for a sidebar, page picker, inspector, and link panel without repeatedly combining raw index and graph files itself.
- [x] Safe page editing API: grow beyond raw `read_page_source` and `write_page_source` into editor-friendly mutations for the parts Fractal owns. The editor should be able to update page body content, title, summary, tags, and notes through library calls that preserve required Fractal structure, rebuild generated data, and report what changed. Raw HTML read/write can remain available, but it should not be the only practical editing path.
- [x] Rename/move page API: support changing a page path and/or title while preserving the unique, case-insensitive label contract. This should fail before writing when the new title or filename stem would collide with another page, move the file safely, rebuild generated data, and provide a clear mutation report. Link repair can stay conservative at first, but the engine should at least make rename/move a coherent operation instead of leaving editors to compose filesystem writes and sync calls.
- [x] Delete page API: provide a first-class operation for removing a page from a project. It should delete the page file, rebuild the generated index and graph, and report affected backlinks/outlinks so an editor can warn the user before or after deletion. The first version can leave ordinary prose untouched, but it should handle Fractal-managed generated data cleanly.
- [x] Editor sync contract: decide and document when editor clients should call `sync`, `index build`, and validation. A basic editor needs predictable rules for save behavior: whether every save rebuilds indexes, whether generated links are applied automatically or by explicit command, and what operation reports mean. This contract matters more than having a UI because it defines how external applications safely cooperate with the engine.

## Editor sync contract

Editor clients should treat the Rust library API as the primary cooperation surface. The CLI is useful for humans and scripts, but an editor should call the crate directly and use `OperationReport` values as the receipt for what the last engine operation changed. `OperationReport` is not a persistent project history; it is returned per operation and contains typed `OperationEvent` values plus a computed `summary()` for UI refresh decisions.

Safe editor mutations rebuild generated data before returning. Calls such as `update_editor_page`, `set_page_title`, `update_page_body`, `set_page_summary`, `set_page_tags`, note mutations, `rename_page`, and `delete_page` update Fractal-owned structure and then rebuild `.fractal/index.json` and `.fractal/graph.json`. Editors do not need to call `index build` after these operations. Reports distinguish source changes from generated-data rebuilds through typed events such as `page_created`, `page_metadata_updated`, `manifest_updated`, `generated_index_built`, and `generated_graph_built`. When editor HTML serializers drop Fractal link markers, `update_editor_page` repairs those invalid links back to readable text before validation so a later explicit sync can regenerate managed links.

Generated links are applied only by explicit sync. Ordinary saves and safe mutation calls do not rewrite prose to add inferred links. Call `sync_project` or the `fractal sync` command when the user asks to refresh generated links, or when the editor has an explicit save policy that includes inferred-link rewriting. `sync_project` rebuilds generated data, rewrites Fractal-managed links inside each page's `<main>`, rebuilds generated data again, and reports how many pages were rewritten.

Use `build_index` when files changed outside the safe mutation APIs and the editor wants fresh index/graph data without rewriting page HTML. This is appropriate after external filesystem edits, asset changes under `pages/`, or raw source writes where generated-link rewriting should not happen yet.

Use `validate_project` as the health gate. Editors should validate when opening a project, after imports or raw source writes, before export/publish workflows, and after detecting external file changes that may have broken Fractal structure. `repair_project(root)` may repair missing Fractal scaffold, page markers, and internal links whose visible text has drifted from the generated target; safe mutation calls are expected to preserve those markers without needing a repair pass.

Raw source APIs remain escape hatches. `read_page_source` and `write_page_source` are available for inspection and advanced tools; `write_page_source` validates candidate HTML before saving and rebuilds generated data if accepted. Callers should still prefer safe mutation APIs for ordinary edits.

## Implicit linking contract

Fractal treats links as generated project structure, not as something users should have to maintain by hand. The current rule is intentionally strict: a link can be inferred only from a known, unique label in the project index.

Page labels are derived from page titles. Matching is normalized and case-insensitive, so `Rust`, `rust`, and `RUST` are the same label for linking and validation purposes. Two pages may not expose the same title label, even if they are stored in different folders. Creating or importing a page with a duplicate title label should fail before writing the new page, and manually-added duplicates should make validation or indexing fail. A page link is only valid when its visible text identifies the target by its title; the `href` is not allowed to become a hidden second source of truth.

Unknown prose is not an unresolved link candidate. If a word or phrase does not match a known page label or page-local note trigger, Fractal leaves it as ordinary text.

## Current CLI surface

The CLI is functional but not yet ergonomically designed. Treat the crate-root Rust API as the primary surface for applications; the CLI migration has only started toward the canonical resource/action grammar from `CLI-PROPOSITION.md`:

```text
fractal [--project <root>] [--format human|json] [--json] <resource> <action> ...

fractal project init <path> [--name <name>]
fractal project validate
fractal project repair
fractal project sync
fractal index build

fractal page list
fractal page read <page/path> [--view agent|metadata|source]
fractal page create <title>
fractal page set <page/path> [--title <title>] [--summary <summary>] [--tag <tag>...] [--body-file <path>]
fractal page move <page/path> --to <new-page/path> [--title <title>]
fractal page delete <page/path> --yes
fractal page source read <page/path>

fractal note add <page/path> <trigger> --content "<content>"
fractal note remove <page/path> <trigger>
fractal note set <page/path> <trigger> --content "<content>"

fractal search text <query>
fractal graph page <page/path>
fractal graph backlinks <page/path>
fractal graph outlinks <page/path>
fractal graph related <page/path>
fractal graph neighbors <page/path> [--depth <n>]
fractal graph notes <page/path>
fractal graph orphans
fractal context page <page/path> [--budget <n>]
fractal import markdown <path/to/file.md>
fractal export markdown <page/path> --to <export/filename.md>
fractal schema commands
```

`--json` returns a stable `fractal.command_result.v1` success envelope for migrated commands. The old top-level `init`, `validate`, and `sync` commands remain as hidden compatibility aliases during the migration.

## Project layout

`fractal init my-project` creates:

```text
my-project/
├── .fractal/
│   └── style.css
├── fractal.json
└── pages/
```

`.fractal/` stores project-owned generated/support files. `style.css` is created during `init`; generated project data such as indexes and graph data also lives there.

`fractal.json` currently stores:

- `project_name`
- `version` (manifest schema version, currently `1`)
- `default_page`
- `theme` (`dark` or `light`)

## What works today

- `init` creates the project folder, manifest, starter stylesheet, and empty `pages/` directory. It does not create a starter page.
- `validate` checks the project structure and enforces the current strict Fractal page contract. Pages must have exactly one direct `<main>` and exactly one direct notes section outside `<main>`, matching `<title>`/first `<main h1>`, exactly the required `fractal:*` meta tags, the exact generated stylesheet href for their depth, a body theme matching the manifest, valid note IDs, allowed body/note elements only, generated links that resolve, generated page-link text that identifies the target title or filename stem, and no manual links or extra `fractal:*` metadata. It warns about ambiguous duplicate page labels for existing files. Creating new duplicate page labels is rejected; behavior with pre-existing duplicates is otherwise undefined for now. HTML extraction for validation is parser-backed, so it is not tied to Fractal's generated indentation or attribute quoting.
- `repair` adds or repairs safe Fractal-owned scaffold pieces before validating: `.fractal/`, `.fractal/style.css`, `pages/`, the configured default page, missing required page meta tags, the generated stylesheet link, the body theme marker, missing title/heading pairs when one side can be inferred, and the notes section. It also merges duplicate notes sections while preserving their child content, unwraps simple manual links into plain text, and rewrites mismatched generated internal page-link text to the visible target title.
- `import` reads a markdown file, converts only basic headings and paragraphs into a minimal HTML page under `pages/`, and rebuilds `.fractal/index.json` and `.fractal/graph.json`. This is an import stub, not real markdown support yet.
- `export` converts only basic headings and paragraphs from an existing Fractal HTML page to markdown at the requested output path. This is an export stub, not real markdown support yet.
- `index build` generates `.fractal/index.json` with every file under `pages/`, page entries for HTML files, page titles, all page meta tags whose names start with `fractal:`, notes, and links. It also generates `.fractal/graph.json` with page/note nodes, graph edges, and per-page backlinks/outlinks.
- `search <query>` reads `.fractal/index.json` and searches page titles, summaries, tags, note labels, and link text. Query words may match across different indexed fields on the same page.
- `graph page <page/path>` reads `.fractal/graph.json` and prints the page's backlinks and outlinks. The page path may be relative to `pages/`, include `pages/`, and omit `.html`.
- `graph backlinks <page/path>` and `graph outlinks <page/path>` print focused page-link views.
- `graph related <page/path>` prints the union of the page's backlinks and outlinks with direction markers.
- `graph neighbors <page/path> --depth <n>` prints a bounded undirected page neighborhood from generated page links.
- `graph notes <page/path>` prints notes contained by the page from the generated graph's `contains_note` edges.
- `graph orphans` reads `.fractal/graph.json` and lists pages with no backlinks.
- `sync` rebuilds `.fractal/index.json` and `.fractal/graph.json`, updates each page's note links inside its own `<main>`, then links remaining matching page-title/page-stem text against the project index. It rebuilds both generated data files again after the page rewrites so generated links are reflected.
- `page new` / `page create` creates a new HTML page from the requested title, normalizes that title to a lowercase kebab-case filename under `pages/`, and rebuilds `.fractal/index.json` and `.fractal/graph.json`.
- Library API: `extract_page_text` returns compact text extracted from the page's `<main>`.
- Library API: `page_metadata`, `set_page_summary`, `set_page_tags`, and `reset_page_metadata` read and mutate Fractal-owned page metadata using parser-backed HTML operations, normalize comma-separated or repeated tags, and rebuild `.fractal/index.json` and `.fractal/graph.json`.
- Library API and CLI: note mutations add/remove/patch notes in the requested page using parser-backed HTML operations and rebuild `.fractal/index.json` and `.fractal/graph.json`.
- Library API: `list_editor_pages` and `editor_page_detail` provide editor sidebars, page inspectors, and link panels without requiring clients to manually combine index and graph files. Editor link details include Fractal-resolved page/note targets so UI clients do not have to reimplement href resolution.
- Library API: `update_editor_page`, `set_page_title`, and `update_page_body` provide parser-backed mutations for Fractal-owned editor fields, repair invalid editor-submitted links back to readable text, validate the resulting Fractal HTML, and rebuild generated data.
- Library API: `write_page_source` validates candidate raw HTML before saving, leaves the original file untouched on validation failure, and rebuilds generated data when accepted.
- Library API: `rename_page` changes a page path and/or title after preflighting the unique label contract, updates the default page manifest entry when needed, updates the page stylesheet link for its new depth, and rebuilds generated data.
- Library API: `delete_page` removes a page, reports its affected backlinks/outlinks, unwraps Fractal-generated links to the deleted page, promotes a replacement default page when deleting the current default, and rebuilds generated data.

All generated pages currently include these required meta tags:

```html
<meta name="fractal:version" content="0.1" />
<meta name="fractal:summary" content="" />
<meta name="fractal:tags" content="" />
```

Generated pages also link to `.fractal/style.css` and set the manifest theme on the body:

```html
<link rel="stylesheet" href="../.fractal/style.css">
<body data-fractal-theme="dark">
```

Notes currently live under a dedicated section in the requested page:

```html
<section data-fractal-notes>
  <aside id="note-java" data-fractal-note>
    <p>my note text</p>
  </aside>
</section>
```

The notes section itself is mandatory and generated for every new page, even when it is empty.

Trigger text is normalized into the note id. For example, `Andromeda Galaxy` becomes `note-andromeda-galaxy`.

`sync` uses those note ids for page-scope linking. A note id such as `note-java` links matching unlinked text in the same page's `<main>` to `#note-java`:

```html
<a href="#note-java" data-fractal-link="note">Java</a>
```

After note links are applied, `sync` uses the project index for project-scope links between pages. Page title labels are case-insensitive and must resolve to exactly one page; duplicate page title labels are invalid for now.

```html
<a href="subpage.html" data-fractal-link="page">Subpage</a>
```

Generated links are marked with `data-fractal-link`, so rerunning `sync` can replace Fractal-managed links. Manual `<a>` links are not part of valid Fractal pages yet; `validate` rejects them, and it rejects generated page links whose visible text does not match the target page title. `repair` may unwrap simple manual links into plain text. Generated internal page links whose text drifted from their target are repaired to show the target title.

`.fractal/index.json` and `.fractal/graph.json` are generated data and include schema versions. They can be regenerated with `fractal index build` or `fractal sync`; graph query commands reject unsupported graph versions rather than guessing.

`.fractal/graph.json` is derived from the parser-backed index. It currently contains:

- page nodes, with ids such as `page:index.html`
- note nodes, with ids such as `note:index.html#note-java`
- `contains_note`, `links_to_note`, and `links_to_page` edges
- per-page `outlinks` and `backlinks` for page-to-page edges

The `import` and `export` flows currently support only markdown headings (`#` through `######`) and plain paragraphs. They are placeholders for the long-term import/export contract, not full markdown compatibility.

## Repo notes

- The active rewrite lives in the root crate.
- [`docs/format-contract.md`](docs/format-contract.md) is the current short-form Fractal project/page contract.
- [`ENGINE-HARDENING-ROADMAP.md`](ENGINE-HARDENING-ROADMAP.md) tracks the engine hardening phases.
- [`CLI-PROPOSITION.md`](CLI-PROPOSITION.md), `wishlist.md`, and `gemma4_wishlists.md` are future-facing design inputs, not descriptions of the current CLI.
- `README-legacy.md` archives older `.frac`/binary-format thinking and should not be treated as current product direction.
