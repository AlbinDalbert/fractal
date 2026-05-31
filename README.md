# Fractal

`fractal` is a reset-in-progress CLI for a file-backed knowledge base / graph engine.

This repo is currently focused on one thing: locking down the first useful command surface and project layout before the deeper format and graph behavior are rebuilt.

## Project direction

Fractal is meant to be better tooling, engine infrastructure, and user experience for linked pages than markdown-oriented knowledge tools such as Obsidian.

Fractal is the engine, not an end-user interface. This crate should focus on the durable file format, project layout, indexing, graph construction, linking, metadata, import/export, search, and programmatic command surface that other tools can build on. A separate UI can consume Fractal data later, but this project should not take on interactive application UI concerns.

The page format is HTML by design. Fractal can impose a strict project structure and strict page conventions, but the user's actual files should remain resilient: if the tool breaks or disappears, the pages are still inspectable, editable, and renderable by ordinary browsers on almost any device. HTML also avoids making Fractal responsible for a custom rendering engine, and gives the project a richer and more consistent rendering target than markdown.

The core product idea is that users should not have to remember which pages to link to, when to link them, or how to maintain the graph by hand. Linking, discovery, metadata, indexing, and graph structure should increasingly be inferred and maintained by the tool.

There is a second-order goal behind the same design: Fractal should become a human-usable knowledge system that is also useful to LLMs, including small local models. The engine should offload graph search, semantic lookup, indexing, summaries, and other ontological work from the model, so an LLM does not need to spend large token budgets rediscovering the structure of a knowledge base. In that sense Fractal is a slim RAG / knowledge-graph engine whose data model is shared with the user-facing linked-page tool.

Because of that, import and export are not side features. Converting other formats into Fractal, and converting Fractal pages back out again, is part of the long-term contract: the project should make knowledge portable while still giving Fractal enough structure to build fast, semantic, programmatic tooling on top.

## Core next tasks

- Backlinks/outlinks graph: expand the initial durable graph data beyond `.fractal/graph.json` v0. This is likely one of the most important engine pieces because runtime linking, discovery, traversal, and LLM-oriented lookup should not depend on repeatedly rescanning every page. The current graph stores page/note nodes, graph edges, and per-page backlinks/outlinks; the next step is to add commands and richer edge types on top of it.
- Search and discovery: add keyword search, graph traversal, related pages, orphan page detection, and unresolved mention tracking.
- Semantic/ontological tooling: support entity extraction, aliases, relationships, concept typing, summaries, and embeddings or other semantic index support.
- Rich metadata model: make summaries and tags editable, replace placeholder defaults, support schema/version migration, and likely introduce stable page IDs or slugs.
- Robust HTML handling: continue replacing string scanning with a proper HTML parser/serializer. Indexing, validation extraction, and note mutation now use an HTML parser, but generated link rewriting and import/export still need a parser-backed mutation/serialization path so ordinary inspectable HTML works without brittle formatting requirements.
- Change tracking/genealogy: track how pages and graph relationships change over time. This may be a later or bonus feature, but it fits the long-term knowledge-engine direction.

## Current CLI surface

```text
fractal init <project-name>
fractal validate
fractal validate --fix
fractal import <path/to/file.md>
fractal export <pages/page.html> <export/filename.md>
fractal index build
fractal sync
fractal page new <page/path>
fractal page <page/path> note add <trigger> "<content>"
fractal page <page/path> note remove <trigger>
fractal page <page/path> note patch <trigger> "<content>"
```

## Project layout

`fractal init my-project` creates:

```text
my-project/
├── .fractal/
│   └── style.css
├── fractal.json
└── pages/
    └── index.html
```

`.fractal/` stores project-owned generated/support files. `style.css` is created during `init`; generated project data such as indexes and graph data also lives there.

`fractal.json` currently stores:

- `project_name`
- `version`
- `default_page`
- `theme` (`dark` or `light`)

## What works today

- `init` creates the project folder, manifest, starter stylesheet, and starter page.
- `validate` checks the project structure, verifies the required Fractal meta tags exist in each page, requires exactly one `data-fractal-notes` section, and rejects malformed note ids. HTML extraction for validation is parser-backed, so it is not tied to Fractal's generated indentation or attribute quoting.
- `validate --fix` adds missing Fractal-owned scaffold pieces before validating: `.fractal/`, `.fractal/style.css`, `pages/`, the configured default page, missing required page meta tags, the generated stylesheet link, the body theme marker, and the notes section. It also merges duplicate notes sections while preserving their child content.
- `import` reads a markdown file, converts basic headings and paragraphs into a minimal HTML page under `pages/`, and rebuilds `.fractal/index.json` and `.fractal/graph.json`.
- `export` converts basic headings and paragraphs from an existing Fractal HTML page to markdown at the requested output path.
- `index build` generates `.fractal/index.json` with every file under `pages/`, page entries for HTML files, page titles, all page meta tags whose names start with `fractal:`, notes, and links. It also generates `.fractal/graph.json` with page/note nodes, graph edges, and per-page backlinks/outlinks.
- `sync` rebuilds `.fractal/index.json` and `.fractal/graph.json`, updates each page's note links inside its own `<main>`, then links remaining matching page-title/page-stem text against the project index. It rebuilds both generated data files again after the page rewrites so generated links are reflected.
- `page new` creates a new HTML page under `pages/`, adds `.html` automatically when omitted, and rebuilds `.fractal/index.json` and `.fractal/graph.json`.
- `page <page/path> note add/remove/patch` mutates notes in the requested page using parser-backed HTML operations and rebuilds `.fractal/index.json` and `.fractal/graph.json`.

All generated pages currently include these required meta tags:

```html
<meta name="fractal:version" content="0.1" />
<meta name="fractal:summary" content="Short page summary here." />
<meta name="fractal:tags" content="rust, graphs, parsing" />
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

After note links are applied, `sync` uses the project index for project-scope links between pages:

```html
<a href="subpage.html" data-fractal-link="page">Subpage</a>
```

Generated links are marked with `data-fractal-link`, so rerunning `sync` can replace Fractal-managed links without changing manual links.

`.fractal/graph.json` is derived from the parser-backed index. It currently contains:

- page nodes, with ids such as `page:index.html`
- note nodes, with ids such as `note:index.html#note-java`
- `contains_note`, `links_to_note`, and `links_to_page` edges
- per-page `outlinks` and `backlinks` for page-to-page edges

The `import` and `export` flows currently support only markdown headings (`#` through `######`) and plain paragraphs.

## Repo notes

- The active rewrite lives in the root crate.
- Older experimental Rust code has been moved into `protptype/` as reference only.
- `README-legacy.md` can hold older ideas if you still want them around during the reset.
- `cargo run --manifest-path ../Cargo.toml --`
