# Fractal Code Map

Use this when the spec is clear but the code feels like fog. This is not a full reference; it is the fastest path to a working mental model.

## One-sentence model

Fractal is a Rust library where public operations compose path resolution, HTML document manipulation, validation, mutation planning, and generated index/graph rebuilds.

## Read order

If you are lost, read in this order:

1. `src/lib.rs` — the public API surface.
2. `src/types.rs` — the data shapes that move across the API.
3. `src/ops/mod.rs` — the operation modules exported by the engine.
4. `src/ops/page.rs` — project/page/import/export/source operations.
5. `src/ops/editor.rs` — editor-facing page list/detail/update operations.
6. `src/ops/mutation.rs` — the write pipeline.
7. `src/document/page.rs` and `src/document/page_links.rs` — parser-backed page edits.
8. `src/index/mod.rs` and `src/graph/mod.rs` — generated derived data.
9. `src/validation.rs` — the format-contract enforcer.
10. `src/cli.rs` — command-line adapter over the library API.

## Directory roles

```text
src/lib.rs              public crate facade
src/types.rs            public DTOs, reports, manifest/index/graph shapes
src/error.rs            FractalError and coarse error codes
src/project/            manifest/layout/path/slug helpers
src/document/           HTML-backed Fractal page reading/mutation helpers
src/ops/                user-visible operations / use cases
src/ops/mutation.rs     central project mutation/write application
src/index/              generated project index
src/graph/              generated graph and graph queries
src/io/                 low-level IO helpers and markdown stub conversion
src/validation.rs       project/page contract validation and repair
src/cli.rs              CLI argument parsing and output adaptation
src/tests.rs            integration-style behavior coverage
```

## The core operation pattern

Most mutating operations should look like this:

```text
load/validate manifest
resolve user path/title input
preflight collisions/invariants
read existing HTML if needed
edit PageDocument or render candidate HTML
validate candidate HTML if needed
MutationPlan::new()
plan writes/moves/removals/events
plan.apply(root)
build_index(root) or write_generated_project_data(root, index)
report.relative_to(root)
```

If you see a user-visible operation that writes files but does not roughly follow this shape, it deserves attention.

## Key flows

### Create project

```text
lib::init_project_at
→ ops/page.rs::init_project_at
→ build ProjectManifest
→ MutationPlan ensures pages/ and .fractal/
→ writes fractal.json and .fractal/style.css
→ OperationEvent::ProjectCreated
```

### Create page

```text
lib::create_page
→ ops/page.rs::create_page
→ project::paths::page_destination_in_directory
→ index::ensure_page_labels_available
→ document::render::render_page_document
→ MutationPlan writes page and maybe manifest default_page
→ index::build_index
  → index::build_project_index
  → graph::build_project_graph
  → write_generated_project_data
```

### Editor page detail

```text
lib::editor_page_detail
→ ops/editor.rs::editor_page_detail
→ resolve_existing_page + page_relative_path
→ PageDocument::parse
→ index::build_project_index
→ graph::build_project_graph
→ return EditorPageDetail:
   source, body_html, metadata, notes, links, backlinks, outlinks
```

This is the main read path Amanite uses to load an active page.

### Save editor page

```text
lib::update_editor_page
→ ops/editor.rs::update_editor_page
→ resolve_existing_page
→ optional title uniqueness preflight
→ PageDocument::set_title / set_main_body_html / set_meta_tag
→ repair invalid editor-submitted links back to readable text
→ validate_page_html_for_project
→ MutationPlan writes changed page and semantic events
→ build_index
```

Important: this rebuilds generated data, but it does not infer new prose links. Explicit sync does that.

### Sync generated links

```text
lib::sync_project
→ ops/sync.rs::sync_project
→ build_project_index
→ for each page:
   read HTML
   unwrap old generated links in <main>
   find note candidates and page-title candidates
   write generated links into text nodes
→ MutationPlan writes changed pages
→ rebuild generated index/graph
→ OperationEvent::SyncCompleted
```

### Rename page

```text
lib::rename_page
→ ops/page.rs::preflight_rename_page
   → resolve source/destination
   → title/path collision checks
   → build index/graph for affected backlinks/outlinks
→ read source page
→ update title, stylesheet href, moved-page internal hrefs
→ plan rewrites in other pages before moving
→ MutationPlan moves/writes destination page and events
→ apply planned rewrites in other pages
→ update manifest default_page if needed
→ build_index
```

This is one of the most important flows to understand because it touches paths, document edits, link repair, manifest updates, generated data, and reports.

### Delete page

```text
lib::delete_page
→ ops/page.rs::preflight_delete_page
   → reject deleting only page
   → collect backlinks/outlinks/default-page impact
→ plan unwrapping links to deleted page in other pages
→ MutationPlan records link impact and removes page
→ apply planned rewrites
→ update manifest default_page if needed
→ build_index
```

### Validate / repair

```text
lib::validate_project
→ validation.rs::validate_project
→ checks manifest/layout/pages/metadata/body/notes/links
→ returns OperationReport with validation event/warnings

lib::repair_project
→ validation.rs::repair_project
→ creates/restores safe scaffold and Fractal-owned markers
→ validates after repair
```

Validation is the contract enforcer. Mutation code should not become a second validation system.

## Mental buckets for files

When opening a file, ask which bucket it is in:

1. **Public surface** — `lib.rs`, `types.rs`, `error.rs`.
2. **Use case** — `ops/*.rs` except `mutation.rs`.
3. **Write pipe** — `ops/mutation.rs`, `io/fs.rs`.
4. **HTML manipulation** — `document/*.rs`.
5. **Path/project rules** — `project/*.rs`.
6. **Derived data** — `index/*.rs`, `graph/*.rs`.
7. **Contract enforcement** — `validation.rs`.
8. **Adapter** — `cli.rs`.

Do not try to understand every helper at once. Follow one operation through these buckets.

## First comprehension drill

Do not refactor. Do this once:

1. Open `src/lib.rs` and pick `rename_page`.
2. Jump to `src/ops/page.rs::rename_page`.
3. Write a 10-line trace in scratch notes of every helper it calls.
4. For each helper, label the bucket: path, document, mutation, index, graph, validation, report.
5. Stop after 30 minutes.

The goal is not mastery. The goal is to teach your brain the route through the code.
