# Agent Notes

- This Rust project is under very active development.
- Prefer the current source, tests, and docs when inferring intended behavior.
- Do not rely on removed legacy prototype code or old custom `.frac`/binary-format design notes unless the user explicitly asks to recover or compare them from git history.
- Preserve the current product direction: Fractal is an HTML-backed linked-page engine, not a markdown-first notes app. Strict Fractal conventions are acceptable, but the user's pages should remain ordinary, inspectable HTML.
- Treat implicit linking, metadata, indexing, summaries, import/export, and semantic/ontological tooling as core engine concerns. They are part of the long-term goal of making Fractal useful both to humans and to LLM workflows, especially small-model workflows that need the tool to offload graph/search work.
