# Fractal CLI Proposition

Status: future-facing CLI design proposal. This is not a description of the current implemented CLI. For the current command surface, see `README.md`; for the current page/project contract, see `docs/format-contract.md`.

This document proposes a clean-slate command-line interface for Fractal, informed by
`LLM-DESIGN-BRIEF.md`, `gemma4_wishlists.md`, the current README, and the current
Rust engine/API direction.

The Gemma4 wishlists were generated from only the design brief. Their strongest
signal is therefore not specific command spelling, but a shared model preference:
small and mid-size LLMs want a resource-oriented interface, explicit state
reports, structured output, graph/context commands, and low-token ways to ask the
engine targeted questions. This proposal treats those preferences as first-class
design pressure while preserving Fractal's broader product direction.

## Design Thesis

Fractal should expose a boring, regular, resource-oriented CLI over a rich engine.
The CLI should feel like a reference tool for the Fractal document/project format,
not like an end-user notes application and not like a generic HTML site tool.

The canonical grammar should be:

```text
fractal [global-options] <resource> <action> [target] [options]
```

Examples:

```text
fractal project validate
fractal page read index --view body
fractal page set index --summary "Project entry point"
fractal graph neighbors index --depth 1
fractal context page index --budget 2000
fractal search text "implicit linking" --limit 10
```

This shape is intentionally plain. It gives humans predictable muscle memory and
gives agents a small grammar to learn.

## Primary Goals

- Make common human workflows easy to discover from `--help`.
- Make agent workflows reliable without regex-parsing human prose.
- Let small local models inspect and mutate large projects without reading every
  file.
- Keep destructive and broad mutations explicit.
- Return stable mutation reports for every write.
- Keep the Rust library API first-class; the CLI should wrap engine operations,
  not become the core abstraction.
- Preserve Fractal's format direction: HTML-backed pages, strict Fractal
  validity, generated graph/index data, and parser-backed mutation.

## Non-Goals

- Do not design an interactive editor inside this crate.
- Do not treat arbitrary HTML as automatically valid Fractal input.
- Do not make markdown the primary document model.
- Do not require LLMs to hand-maintain graph invariants that the engine can infer.
- Do not make semantic/embedding support depend on remote services.

## Gemma4 Wishlist Synthesis

The four Gemma4 responses converge on these design requirements:

| Requirement | CLI implication |
| --- | --- |
| Resource-oriented hierarchy | Use `project`, `page`, `note`, `graph`, `search`, `context`, `import`, `export`, `schema`. |
| Predictable verbs | Prefer `list`, `read`, `create`, `set`, `add`, `remove`, `move`, `delete`, `validate`, `repair`, `build`, `sync`. |
| Machine-readable output | Every command supports `--format json`; write commands return structured reports. |
| Low-token context | Add dedicated `context` commands with budgets, summaries, snippets, and graph neighborhoods. |
| Explicit mutation reports | Every write returns what changed, what was rebuilt, and what relationships were affected. |
| Validation and repair | First-class `project validate` and `project repair`; validation errors use stable codes. |
| Graph-first operations | Expose backlinks, outlinks, neighbors, paths, orphans, weak areas, and graph export. |
| Agent capability discovery | Add `schema` commands so agents can inspect commands, outputs, errors, and data shapes. |
| Safety | Support `--dry-run`, preflight checks, explicit delete confirmation, and atomic operation intent. |
| Semantic future | Reserve `search semantic`, entity extraction, aliases, summaries, and embeddings as core engine concerns. |

## Global Options

These options should work consistently across the CLI.

```text
--project <root>            Project root. Defaults to current directory.
--format human|json|jsonl   Output format. Defaults to human for terminals.
--json                      Alias for --format json.
--no-color                  Disable terminal color.
--quiet                     Print only the requested value or machine output.
--verbose                   Include diagnostics and timing.
--dry-run                   Preflight and report intended changes without writing.
--yes                       Confirm destructive or broad operations in non-interactive use.
--limit <n>                 Limit result count where applicable.
--fields <list>             Return only selected fields in structured output.
--budget <tokens-or-chars>  Soft output budget for context/snippet commands.
```

`--format json` should be valid for every command, including commands that are
simple today. Small agents should never need to parse human prose.

Do not reuse `--format` for domain conversions such as export file type or graph
file type. Use subcommands such as `export markdown` or an operation-local flag
such as `--as dot` so agents can treat `--format` as the result serialization
mode everywhere.

## Output Contract

JSON output should use a stable envelope:

```json
{
  "ok": true,
  "schema": "fractal.command_result.v1",
  "command": "page.set",
  "project": {
    "root": "/path/to/project"
  },
  "data": {},
  "report": {
    "mode": "applied",
    "events": []
  },
  "diagnostics": []
}
```

Errors should also be structured:

```json
{
  "ok": false,
  "schema": "fractal.command_result.v1",
  "command": "project.validate",
  "error": {
    "code": "ERR_DUPLICATE_PAGE_LABEL",
    "message": "duplicate page label `rust` for intro.html and rust.html",
    "severity": "error",
    "paths": ["pages/intro.html", "pages/rust.html"]
  },
  "diagnostics": []
}
```

Recommended stable error code families:

```text
ERR_PROJECT_NOT_FOUND
ERR_MANIFEST_INVALID
ERR_PAGE_NOT_FOUND
ERR_PAGE_ALREADY_EXISTS
ERR_INVALID_FRACTAL_PAGE
ERR_UNSUPPORTED_FORMAT_VERSION
ERR_DUPLICATE_PAGE_LABEL
ERR_DUPLICATE_NOTE_ID
ERR_LINK_TARGET_AMBIGUOUS
ERR_LINK_TARGET_MISSING
ERR_GRAPH_STALE
ERR_GRAPH_VERSION_UNSUPPORTED
ERR_INDEX_STALE
ERR_DRY_RUN_REQUIRED
ERR_CONFIRMATION_REQUIRED
```

## Mutation Report Contract

Every state-changing command should return an `OperationReport`, in human and
JSON modes. The report should be the main agent verification primitive.

Report events should be specific and semantic, not just file diffs:

```text
created_page
moved_page
deleted_page
updated_page_title
updated_page_body
updated_metadata
added_note
removed_note
patched_note
updated_links
affected_links
built_index
built_graph
synced_page
validated_project
repaired_project
```

Dry runs should return the same report shape, with `mode: "dry_run"` and
`would_write: true` on events that would modify files.

For broad operations, reports should include counts and affected paths:

```json
{
  "mode": "applied",
  "events": [
    {
      "type": "moved_page",
      "from": "pages/old.html",
      "to": "pages/new.html"
    },
    {
      "type": "updated_links",
      "page": "pages/index.html",
      "count": 3
    },
    {
      "type": "built_index",
      "path": ".fractal/index.json"
    },
    {
      "type": "built_graph",
      "path": ".fractal/graph.json"
    }
  ]
}
```

## Page and Node Identity

Commands should accept page references in a strict, documented order:

1. Path relative to `pages/`: `index`, `index.html`, `folder/topic`.
2. Project-relative path: `pages/index.html`.
3. Future stable page IDs: `page:<id>` or `id:<id>`.

Graph node IDs should be accepted by graph commands:

```text
page:index.html
note:index.html#note-java
```

Titles should not be accepted as implicit page selectors by default. They are
human-friendly but create avoidable ambiguity for agents. If title lookup is
added, require an explicit selector such as:

```text
fractal page read --title "Project Index"
```

Search results, graph results, and context results should always return both the
human path and the stable ID when stable IDs exist.

## Resource Hierarchy

### `fractal project`

Project-level lifecycle, contract, and broad maintenance.

```text
fractal project init <path> [--name <name>] [--theme dark|light]
fractal project status [--format json]
fractal project validate [--strict] [--format json]
fractal project repair [--scope scaffold|pages|links|generated|all] [--dry-run]
fractal project sync [--links] [--index] [--graph] [--dry-run]
fractal project audit [--format json]
fractal project info [--format json]
```

Notes:

- `project validate` checks the Fractal contract and should not mutate.
- `project repair` is the mutating counterpart to `validate --fix`.
- `project sync` rebuilds generated data and optionally rewrites Fractal-managed
  links. It should clearly report whether page HTML was rewritten.
- `project audit` is for graph health: orphan pages, duplicate labels, weakly
  connected areas, link density, stale generated data, and unsupported schema
  versions.

### `fractal page`

Page creation, reading, safe mutation, movement, deletion, and source escape
hatches.

```text
fractal page list [--summary] [--tags] [--graph-facts] [--format json]
fractal page read <page> [--view full|body|metadata|notes|links|source|agent]
fractal page create <page> [--title <title>] [--summary <text>] [--tag <tag>...]
fractal page set <page> [--title <title>] [--summary <text>] [--tag <tag>...] [--body-file <path>]
fractal page move <page> --to <new-page> [--title <title>] [--dry-run]
fractal page delete <page> [--dry-run] [--yes]
fractal page source read <page>
fractal page source write <page> --from <path> [--validate]
```

`page read --view agent` should return the LLM-friendly view of one page: path,
title, summary, tags, notes, outgoing links, backlinks, and body text stripped of
HTML boilerplate.

`page set` is intentionally narrow. It should mutate Fractal-owned fields and
safe page body content through parser-backed operations. Raw source writes remain
available but should be visibly marked as an escape hatch.

When `page set` receives one or more `--tag` values, it should replace the tag
list. Additive tag editing can be added later as `page tag add` and
`page tag remove` if that becomes common enough to justify more commands.

### `fractal note`

Notes are page-local, but they are graph nodes. A top-level `note` resource makes
agent usage less nested and easier to discover.

```text
fractal note list <page> [--format json]
fractal note read <page> <trigger-or-id> [--format json]
fractal note add <page> <trigger> (--content <text> | --content-file <path>)
fractal note set <page> <trigger-or-id> (--content <text> | --content-file <path>)
fractal note remove <page> <trigger-or-id> [--dry-run]
```

The CLI can keep `page note ...` as a compatibility alias, but the canonical
agent-facing form should be `note <action> <page>`.

### `fractal link`

Generated links are important enough to expose separately from project sync.

```text
fractal link candidates [<page>] [--format json]
fractal link apply [<page>] [--dry-run]
fractal link check [<page>] [--format json]
fractal link labels [--format json]
```

Notes:

- `link candidates` shows known unambiguous page/note labels that could be
  linked.
- `link apply` rewrites only Fractal-managed links and leaves manual links alone.
- `link check` reports unresolved generated targets, ambiguous labels, and stale
  generated links.
- `link labels` is useful to agents before creating or renaming pages because it
  exposes the current case-insensitive label space.

### `fractal index`

Generated project data management.

```text
fractal index build [--format json]
fractal index status [--format json]
fractal index read [--format json]
```

`index build` should rebuild both `.fractal/index.json` and `.fractal/graph.json`,
matching the current engine behavior. `index status` should report whether
generated data is missing, stale, or unsupported.

### `fractal graph`

Graph traversal, structural discovery, and relationship queries.

```text
fractal graph page <page> [--format json]
fractal graph backlinks <page> [--format json]
fractal graph outlinks <page> [--format json]
fractal graph related <page> [--format json]
fractal graph neighbors <page> [--depth <n>] [--direction in|out|both]
fractal graph notes <page> [--format json]
fractal graph orphans [--format json]
fractal graph path <from-page> <to-page> [--max-depth <n>]
fractal graph query --pattern <pattern> [--format json]
fractal graph export --as json|dot
```

`neighbors` is the main small-model graph command. It should return a bounded
local neighborhood without forcing an agent to load the entire graph.

`query --pattern` can start simple and grow. Early patterns could be named:

```text
orphan
backlinkless
tag:<tag>
links_to:<page>
linked_from:<page>
weakly_connected
```

### `fractal search`

Search is the main discovery interface. It should support keyword search today
and reserve a clear semantic path for local embeddings later.

```text
fractal search text <query> [--field title|summary|tags|notes|links|body] [--limit <n>]
fractal search metadata [--title <text>] [--tag <tag>] [--summary <text>]
fractal search semantic <query> [--limit <n>] [--provider local] [--model <name>]
fractal search all <query> [--limit <n>]
```

The default result shape should include:

```text
path
id
title
summary
tags
score or rank
matched fields
small snippets
```

Semantic search should be local-first. Remote embedding providers can be added
later, but the CLI should not assume network access or hosted services.

### `fractal context`

This is the most explicitly LLM/agent-oriented resource. Its job is to convert
Fractal's graph and index into compact, bounded context packets.

```text
fractal context page <page> [--budget <n>] [--include body,meta,notes,links,backlinks]
fractal context neighborhood <page> [--depth <n>] [--budget <n>]
fractal context search <query> [--budget <n>] [--limit <n>]
fractal context project [--budget <n>]
```

Recommended behavior:

- Prefer summaries, tags, titles, note labels, and snippets before raw full HTML.
- Include backlinks/outlinks as compact adjacency lists.
- Include "next read" suggestions when available.
- Respect the budget by truncating snippets deterministically.
- Return enough provenance for every snippet that an agent can request the full
  page next.

Example compact JSON shape:

```json
{
  "page": {
    "path": "index.html",
    "title": "Project Index",
    "summary": "Entry point for the project.",
    "tags": ["fractal", "overview"]
  },
  "snippets": [
    {
      "source": "body",
      "text": "Fractal treats generated links as project structure..."
    }
  ],
  "neighbors": {
    "outlinks": [{"page": "linking.html", "text": "implicit linking"}],
    "backlinks": [{"page": "roadmap.html", "text": "project index"}]
  }
}
```

### `fractal import`

Import is a core boundary operation: external content becomes valid Fractal.

```text
fractal import markdown <source> [--to <page>] [--title <title>] [--tag <tag>...]
fractal import html <source> [--to <page>] [--repair] [--mapping <schema-file>]
fractal import directory <source-dir> --from markdown|html [--dry-run]
```

Import should validate the resulting Fractal pages before committing writes when
possible. For agents, dry-run import reports are important because imports can
create many pages and labels.

### `fractal export`

Export converts Fractal back out while preserving useful structure.

```text
fractal export markdown <page> --to <path>
fractal export html <page-or-project> --to <path>
fractal export json <page-or-project> --to <path>
fractal export graph --to <path> --as json|dot
```

Exports should be explicit about whether they are exporting user-authored content,
Fractal metadata, generated links, graph data, or all of the above.

### `fractal schema`

Agents need cheap capability discovery.

```text
fractal schema commands [--format json]
fractal schema command <resource.action> [--format json]
fractal schema output <resource.action> [--format json]
fractal schema errors [--format json]
fractal schema project [--format json]
fractal schema page [--format json]
```

This lets an agent ask the tool what it can do without reading the whole README
or scraping help text.

The schema should include:

```text
canonical command name
accepted arguments
accepted flags
read/write classification
dry-run support
output schema
possible error codes
examples
```

## Command Naming Rules

Use stable, boring verbs:

```text
list
read
create
set
add
remove
move
delete
validate
repair
build
sync
status
audit
search
neighbors
context
export
import
```

Avoid clever verbs and overloaded aliases in the canonical interface. Aliases can
exist for humans, but `schema commands` should clearly mark one spelling as
canonical.

## Human Defaults vs Agent Defaults

Default terminal output should be compact human text.

Example:

```text
updated fractal:summary for pages/index.html
built .fractal/index.json
built .fractal/graph.json
```

Agent calls should use:

```text
--format json
--fields ...
--limit ...
--budget ...
```

Do not make complex graph/search commands default to huge output. Large outputs
hurt both humans and small models. Require explicit `--full` or larger limits for
bulk data.

## Safety Rules

- `validate` never writes.
- `repair`, `sync`, `link apply`, `page move`, `page delete`, imports, and raw
  source writes support `--dry-run`.
- `page delete` requires `--yes` outside an interactive confirmation path.
- Broad rewrites should report affected pages before writing in dry-run mode.
- Mutations should preflight project invariants before the first write whenever
  possible.
- Failed mutation commands should not silently leave generated data stale.
- Raw source writes should recommend or optionally run validation.

## Agent Workflows

### Find the right page without reading everything

```text
fractal search text "implicit linking" --format json --limit 5
fractal context neighborhood linking.html --depth 1 --budget 2000 --format json
```

### Safely update metadata

```text
fractal page read linking.html --view metadata --format json
fractal page set linking.html --summary "Rules for generated Fractal links." --tag graph --tag links --format json
fractal project validate --format json
```

### Rename a page with preflight

```text
fractal page move old-topic --to new-topic --title "New Topic" --dry-run --format json
fractal page move old-topic --to new-topic --title "New Topic" --format json
```

The dry run should report label collisions, affected backlinks, outgoing links,
manifest updates, and generated data rebuilds.

### Refresh generated links

```text
fractal link candidates --format json
fractal link apply --dry-run --format json
fractal link apply --format json
```

### Build a small local RAG context packet

```text
fractal context search "Fractal validation repair rules" --budget 3000 --limit 6 --format json
```

This should return the smallest useful set of summaries, snippets, paths, tags,
and graph facts needed for the model's next step.

## Current CLI Migration

The current CLI already contains many of the right engine concepts. The main
change is regularizing them under resource/action grammar and adding structured
output.

| Current command | Proposed canonical command |
| --- | --- |
| `fractal init <name>` | `fractal project init <path> --name <name>` |
| `fractal validate` | `fractal project validate` |
| `fractal validate --fix` | `fractal project repair --scope all` |
| `fractal sync` | `fractal project sync` or `fractal link apply` depending on scope |
| `fractal index build` | `fractal index build` |
| `fractal search <query>` | `fractal search text <query>` |
| `fractal graph page <page>` | `fractal graph page <page>` |
| `fractal graph related <page>` | `fractal graph related <page>` or `fractal graph neighbors <page>` |
| `fractal graph neighbors <page> --depth <n>` | `fractal graph neighbors <page> --depth <n>` |
| `fractal page new <page>` | `fractal page create <page>` |
| `fractal page <page> extract` | `fractal page read <page> --view body` or `fractal context page <page>` |
| `fractal page <page> meta show` | `fractal page read <page> --view metadata` |
| `fractal page <page> meta set-summary <text>` | `fractal page set <page> --summary <text>` |
| `fractal page <page> meta set-tags <tags...>` | `fractal page set <page> --tag <tag>...` |
| `fractal page <page> note add <trigger> <content>` | `fractal note add <page> <trigger> --content <text>` |
| `fractal import <file.md>` | `fractal import markdown <file.md>` |
| `fractal export <page> <file.md>` | `fractal export markdown <page> --to <file.md>` |

Compatibility aliases can remain while the project is pre-alpha, but docs and
machine schemas should present only the canonical shape.

## CLI Migration Stages

These stages are CLI migration stages, not the engine hardening phases from `ENGINE-HARDENING-ROADMAP.md`.

### Stage 1: Regularize and expose reports

- Add global `--project`, `--format`, `--json`, `--quiet`, and `--no-color`.
- Serialize existing `OperationReport` in JSON mode.
- Add stable JSON envelopes for success and failure.
- Add `project` namespace aliases for `init`, `validate`, `repair`, and `sync`.
- Add `schema commands` with static command metadata.

### Stage 2: Make existing library API visible

- Expose `page list`, `page read`, `page set`, `page move`, and `page delete`.
- Expose `note list/read/add/set/remove`.
- Add dry-run support to move/delete/repair/sync/link operations where practical.
- Regularize `graph neighbors` under the canonical JSON-capable graph surface.
- Add `context page` and `context neighborhood` from existing index/graph/page
  detail data.

### Stage 3: Agent search and ontological growth

- Add snippets from parsed body text to search results.
- Add `context search`.
- Add entity/alias/relationship extraction reports.
- Add local-first `search semantic` and embedding index commands.
- Add richer graph query patterns and graph export formats.

### Stage 4: Stronger safety and transactions

- Preflight all multi-file operations before writes.
- Add operation journals or backups for broad rewrites.
- Add stronger stale index/graph detection.
- Add structured repair plans that can be reviewed, then applied.

## Recommended First Canonical Surface

If the project wants the smallest useful near-term CLI that matches the Gemma
signal, prioritize these commands first:

```text
fractal project init <path> [--name <name>]
fractal project validate [--format json]
fractal project repair [--dry-run] [--format json]
fractal project sync [--dry-run] [--format json]

fractal page list [--format json]
fractal page read <page> --view agent [--format json]
fractal page create <page> [--title <title>] [--summary <text>] [--tag <tag>...]
fractal page set <page> [--title <title>] [--summary <text>] [--tag <tag>...]
fractal page move <page> --to <new-page> [--dry-run] [--format json]
fractal page delete <page> [--dry-run] [--yes] [--format json]

fractal note add <page> <trigger> --content <text> [--format json]
fractal note set <page> <trigger-or-id> --content <text> [--format json]
fractal note remove <page> <trigger-or-id> [--dry-run] [--format json]

fractal search text <query> [--limit <n>] [--format json]
fractal graph neighbors <page> --depth <n> [--format json]
fractal graph orphans [--format json]
fractal context page <page> --budget <n> [--format json]
fractal schema commands [--format json]
```

This subset gives small local agents a dependable loop:

1. Discover commands with `schema`.
2. Search or ask for bounded context.
3. Mutate through safe resource commands.
4. Read the mutation report.
5. Validate the project.

That loop is the central CLI design goal.
