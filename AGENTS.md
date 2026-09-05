# Agent Notes

- This Rust project is under very active development.
- Follow `docs/native-only-cleanup-plan.md` as the sole roadmap for the native-only v2 cleanup. During that cleanup, use the plan to determine direction and use the current source, tests, and public docs to understand the implementation that has landed so far. Do not rewrite the public format contract ahead of the plan's final documentation commit.
- When making changes to any rust files, verify the project state before handoff with `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`. If a command cannot be run or fails because of pre-existing worktree state, report that explicitly.
- Do not rely on removed legacy prototype code or old custom `.frac`/binary-format design notes unless the user explicitly asks to recover or compare them from git history.
- Fractal is the engine for projects made of native Fractal documents. It is not a Markdown notes app, an arbitrary-HTML site tool, or an end-user editor.
- Fractal documents use HTML as their storage representation, but valid Fractal is stricter than valid HTML. Native `*.fractal.html` documents remain inspectable, editable, and renderable with common HTML tools while Fractal owns their format contract.
- Fractal recognizes the root `fractal.json`, `.fractal.lock`, directories below `pages/`, native `*.fractal.html` documents, and folder `fractal.json` metadata. Treat every other file as opaque. Do not expose opaque files as pages or assets, and do not silently delete or relocate them.
- Keep the Rust library API first-class and the CLI thin. Callers must be able to use project and document operations directly through the crate.
- Retain project initialization, inspection, validation, recovery, and repair; native page and folder mutations; recoverable transactions, conflict detection, and mutation receipts; the in-memory native document catalog and native link index; native text search, stored links, backlinks, exact-title derived links, and explicit native link insertion; and the current single-page and ordered-folder HTML exports.
- Keep read operations non-writing. Route normal project mutations through the common lock, change plan, transaction, and receipt implementation. Native semantic edits must not replace a complete source file.
- Do not add import or legacy-conversion work, raw HTML pages, assets, embeds, summaries, semantic analysis, ontology, context packets, token budgets, persistent indexes or graph files, persistent mutation history, or generalized command, extension, transaction, or compiler frameworks.
- Keep application-specific work, including Amanite implementation and UI policy, outside this repository.
