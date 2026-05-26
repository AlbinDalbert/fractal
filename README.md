# Fractal

`fractal` is a reset-in-progress CLI for a file-backed knowledge base / graph engine.

This repo is currently focused on one thing: locking down the first useful command surface and project layout before the deeper format and graph behavior are rebuilt.

## Current CLI surface

```text
fractal init <project-name>
fractal validate
fractal validate --fix
fractal import <path/to/file.md>
fractal export <pages/page.html> <export/filename.md>
fractal index build
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

`.fractal/` stores project-owned generated/support files. `style.css` is created during `init`; generated project data such as indexes also lives there.

`fractal.json` currently stores:

- `project_name`
- `version`
- `default_page`
- `theme` (`dark` or `light`)

## What works today

- `init` creates the project folder, manifest, starter stylesheet, and starter page.
- `validate` checks the project structure, verifies the required Fractal meta tags exist in each page, and requires a `data-fractal-notes` section.
- `validate --fix` adds missing Fractal-owned scaffold pieces before validating: `.fractal/`, `.fractal/style.css`, `pages/`, the configured default page, missing required page meta tags, the generated stylesheet link, the body theme marker, and the notes section.
- `import` reads a markdown file and wraps it in a minimal HTML page under `pages/`.
- `export` currently copies an existing Fractal HTML page to the requested output path.
- `index build` generates `.fractal/index.json` with page paths relative to `pages/` and all page meta tags whose names start with `fractal:`.
- `page new` creates a new HTML page under `pages/`, adds `.html` automatically when omitted, and rebuilds `.fractal/index.json`.
- `page <page/path> note add/remove/patch` mutates notes in the requested page and rebuilds `.fractal/index.json`.

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

The `import` and `export` flows are intentionally scaffolding-level right now. They establish the contract and file layout, but they are not yet the final format transformation pipeline.

## Repo notes

- The active rewrite lives in the root crate.
- Older experimental Rust code has been moved into `protptype/` as reference only.
- `README-legacy.md` can hold older ideas if you still want them around during the reset.
- `cargo run --manifest-path ../Cargo.toml --`
