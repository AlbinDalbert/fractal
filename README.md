# Fractal

`fractal` is a reset-in-progress CLI for a file-backed knowledge base / graph engine.

This repo is currently focused on one thing: locking down the first useful command surface and project layout before the deeper format and graph behavior are rebuilt.

## Current CLI surface

```text
fractal init <project-name>
fractal validate
fractal import <path/to/file.md>
fractal export <pages/page.html> <export/filename.md>
```

## Project layout

`fractal init my-project` creates:

```text
my-project/
├── fractal.json
└── pages/
    └── index.html
```

`fractal.json` currently stores:

- `project_name`
- `version`
- `default_page`

## What works today

- `init` creates the project folder, manifest, and starter page.
- `validate` checks that the current directory looks like a Fractal project.
- `import` reads a markdown file and wraps it in a minimal HTML page under `pages/`.
- `export` currently copies an existing Fractal HTML page to the requested output path.

The `import` and `export` flows are intentionally scaffolding-level right now. They establish the contract and file layout, but they are not yet the final format transformation pipeline.

## Repo notes

- The active rewrite lives in the root crate.
- Older experimental Rust code has been moved into `protptype/` as reference only.
- `README-legacy.md` can hold older ideas if you still want them around during the reset.
