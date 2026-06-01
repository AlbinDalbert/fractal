# Agent Notes

- This Rust project is under very active development.
- Prefer the current source, tests, and docs when inferring intended behavior.
- Do not rely on removed legacy prototype code or old custom `.frac`/binary-format design notes unless the user explicitly asks to recover or compare them from git history.
- Preserve the current product direction: Fractal is the engine for the Fractal document/project format, not a markdown-first notes app and not a generic arbitrary-HTML site tool.
- Fractal documents are HTML-backed at the raw storage layer, but "valid Fractal" is stricter than "valid HTML". Think of this like JSON borrowing JavaScript syntax, or Word documents borrowing XML: the underlying representation is inspectable, but Fractal owns the format contract.
- User pages should remain ordinary enough to inspect, edit, and render with common HTML tools, but do not frame arbitrary HTML as first-class Fractal input. External HTML, markdown, or other sources may be imported/converted into Fractal; they are not automatically Fractal.
- The intended scope is closer to an FFmpeg-style reference engine for Fractal: create, validate, repair, mutate, index, link, query, import, and export Fractal projects and documents. Avoid drifting into end-user app/UI concerns unless explicitly requested.
- Treat implicit linking, metadata, indexing, summaries, import/export, and semantic/ontological tooling as core engine concerns. They are part of the long-term goal of making Fractal useful both to humans and to LLM workflows, especially small-model workflows that need the tool to offload graph/search work.
