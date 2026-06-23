# Fractal Feature Inventory

Status: current code inventory, 2026-06-23. This answers: "what does this repo actually support today?" It is intentionally more concrete than the product/spec docs.

Legend:

- **Landed**: implemented and covered by tests or direct code paths.
- **Stub**: implemented narrowly as a placeholder, not full long-term behavior.
- **Partial**: usable, but known contract/ergonomics/freshness gaps remain.

## Project and manifest

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| Initialize a project layout | Landed | `init_project`, `init_project_at` | `fractal project init <path>` | `src/ops/page.rs` | Creates `fractal.json`, `.fractal/style.css`, and `pages/`. Does not create starter page in Fractal itself. |
| Load project manifest | Landed | `load_project_manifest` | indirect | `src/project/paths.rs` | Validates manifest version. |
| Project summary / freshness check | Partial | `project_summary` | none currently | `src/ops/summary.rs` | Reports validation state, counts, generated index/graph existence and freshness. Freshness is not yet enforced everywhere. |
| Validate project | Landed | `validate_project` | `fractal project validate` | `src/validation.rs` | Enforces current format contract. Returns operation report. |
| Repair project/page scaffold | Landed | `repair_project`, `preflight_repair_project` | `fractal project repair` | `src/validation.rs` | Repairs safe Fractal-owned scaffold/markers only; not arbitrary HTML conversion. |

## Page files and directories

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| Create directory under `pages/` | Landed | `create_directory` | none direct except page delete-folder for deletion | `src/ops/page.rs` | Path components must be safe slugs. |
| Delete directory under `pages/` | Landed | `delete_directory` | `fractal page delete-folder <folder> --recursive --yes` | `src/ops/page.rs` | Deletes nested pages, unwraps deleted links, updates default page if needed, rebuilds generated data. |
| Create page | Landed | `create_page`, `new_page` | `fractal page create <page>` | `src/ops/page.rs` | Title/path label collision checks, optional directory, default-page update if first page, rebuilds index/graph. |
| Rename/move page | Landed | `rename_page`, `preflight_rename_page`, `set_page_title` | `fractal page move <page> --to <path> [--title <title>]` | `src/ops/page.rs`, `src/ops/editor.rs` | Preflights collisions and graph impact; moves file, updates title/stylesheet/default page/link text/hrefs, rebuilds generated data. |
| Delete page | Landed | `delete_page`, `preflight_delete_page` | `fractal page delete <page> --yes` | `src/ops/page.rs` | Rejects deleting the only page, unwraps links to deleted page, updates default page, rebuilds generated data. |
| Read raw page source | Landed | `read_page_source` | `fractal page source read <page>` | `src/ops/page.rs` | Escape hatch. |
| Write raw page source | Landed | `write_page_source` | no direct CLI write command | `src/ops/page.rs` | Validates candidate HTML before saving; rebuilds generated data. |
| Extract compact page text | Landed | `extract_page_text` | indirect | `src/ops/page.rs`, `src/document/page.rs` | Used by Amanite preview and search helpers. |

## Editor-facing page API

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| List pages for editor sidebar | Landed | `list_editor_pages` | `fractal page list` | `src/ops/editor.rs` | Includes path, title, summary, tags, backlink/outlink counts. |
| Read editor page detail | Landed | `editor_page_detail` | `fractal page read <page> [--view agent|metadata|source]` | `src/ops/editor.rs` | Returns source, body HTML, metadata, notes, links, backlinks, outlinks. |
| Update editor page fields | Landed | `update_editor_page`, `update_page_body` | `fractal page set <page> [--title] [--summary] [--tag] [--body-file]` | `src/ops/editor.rs` | Updates Fractal-owned fields, repairs editor-submitted broken generated links to text, validates, rebuilds generated data. |

## Metadata and notes

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| Read page metadata | Landed | `page_metadata`, `page_metadata_report` | `fractal page read --view metadata` | `src/document/metadata.rs`, `src/ops/editor.rs` | Reads title, summary, tags, fractal meta. |
| Set/reset summary and tags | Landed | `set_page_summary`, `set_page_tags`, `reset_page_metadata` | mostly through `fractal page set` | `src/document/metadata.rs` | Rebuilds generated data. |
| Add note | Landed | `add_note` | `fractal note add <page> <trigger> --content ...` | `src/document/notes.rs` | Note ID derived from trigger. |
| Patch note | Landed | `patch_note` | `fractal note set <page> <trigger> --content ...` | `src/document/notes.rs` | Replaces note body. |
| Remove note | Landed | `remove_note` | `fractal note remove <page> <trigger>` | `src/document/notes.rs` | Leaves required notes section present. |

## Generated data, links, search, graph

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| Build generated index and graph | Landed | `build_index`, `load_project_index`, `load_project_graph` | `fractal index build` | `src/index/mod.rs`, `src/graph/mod.rs` | Writes `.fractal/index.json` and `.fractal/graph.json` only when bytes change. |
| Sync generated links | Landed | `sync_project` | `fractal project sync` | `src/ops/sync.rs` | Rebuilds index, rewrites generated note/page links in page `<main>`, rebuilds index/graph again. |
| Implicit note/page linking | Landed | through `sync_project` | through `project sync` | `src/ops/sync.rs`, `src/graph/links.rs` | Page-local notes are preferred over same-named pages. Skips code/pre/manual generated contexts. |
| Search indexed fields | Landed | `search_project`, `search_report` | `fractal search text <query>` | `src/index/search.rs` | Searches titles, summaries, tags, note labels, link text, and page body text. |
| Page graph view | Landed | `graph_page`, `page_backlinks`, `page_outlinks`, reports | `fractal graph page/backlinks/outlinks` | `src/graph/mod.rs` | Reads generated graph file. |
| Related/neighbors/orphans/notes graph queries | Landed | `related_pages`, `neighbor_pages`, `orphan_pages`, `page_notes` | `fractal graph related/neighbors/orphans/notes` | `src/graph/mod.rs` | Depth-limited undirected neighbors implemented. |
| Generated data freshness enforcement | Partial | `project_summary` only | none direct | `src/ops/summary.rs` | Roadmap Phase 5: graph/search reads can still use stale generated files unless caller rebuilds/checks. |

## Import/export/context

| Capability | Status | Public/API entry | CLI entry | Main code | Evidence / notes |
|---|---:|---|---|---|---|
| Import markdown | Stub | `import_markdown` | `fractal import markdown <source.md>` | `src/ops/page.rs`, `src/io/markdown.rs` | Supports headings and paragraphs only. Rebuilds generated data. |
| Export markdown | Stub | `export_page` | `fractal export markdown <page> --to <path>` | `src/ops/page.rs`, `src/io/markdown.rs` | Supports basic headings/paragraphs from existing page. |
| Page context packet | Partial | CLI-only composition of existing APIs | `fractal context page <page> [--budget <n>]` | `src/cli.rs` | Returns `EditorPageDetail` plus depth-1 neighbors; budget is carried in output but not yet used to trim context. |
| CLI command schema | Partial | CLI-only | `fractal schema commands` | `src/cli.rs` | Lists canonical commands/examples for machine consumers. |

## Format support boundaries

| Area | Current support |
|---|---|
| Page storage | HTML-backed Fractal pages, not arbitrary HTML. |
| Valid body elements | Small Phase 1 subset: paragraphs, h2-h6, lists, blockquote, pre/code, generated links. |
| Manual links | Invalid in normal validation; repair may unwrap simple manual links. |
| External links | Not first-class valid Fractal links yet. |
| Markdown | Import/export stubs only, not full markdown compatibility. |
| Page identity | Path-first today. Stable internal IDs are an open roadmap question. |

## What this repo does not support yet

- Full arbitrary HTML as valid Fractal input.
- Rich Markdown import/export.
- External web links as first-class Fractal links.
- Stable internal page IDs / rename history.
- Enforced generated-data freshness on all graph/search reads.
- Rich semantic search, embeddings, aliases, entities, typed relationships, or LLM context budgeting.
- A user-facing desktop UI; that is Amanite.
