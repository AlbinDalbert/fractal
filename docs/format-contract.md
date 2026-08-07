# Fractal Format Contract

Fractal projects are directories of ordinary linked HTML documents.

## Project

A project contains:

- `fractal.json`;
- a `pages/` directory.

The manifest has two fields:

```json
{
  "name": "Project name",
  "version": 1
}
```

`name` must not be empty. `version` must be supported by the engine.

There is no required `.fractal/` directory and no persistent index or graph data.

## Pages

Every page:

- is a UTF-8 file below `pages/`;
- has a relative path ending in `.html`;
- has a non-empty title identifiable from `<title>` or, as a fallback, the first `<h1>`.

Pages may use ordinary HTML. Fractal does not prescribe body structure, metadata, styling, themes, or an element subset. It does not require generated attributes or sections.

Paths supplied to the engine must remain below `pages/`; absolute paths and parent traversal are rejected. Paths—not titles—identify pages. Duplicate titles are therefore allowed and naturally produce multiple link candidates.

## Links

Links are ordinary `<a href="…">` elements.

- Relative internal links are resolved from the source page directory.
- Root-relative links are resolved from `pages/`.
- Fragment-only links are retained as local fragments.
- URI schemes and protocol-relative URLs are external.
- Relative links to existing non-page files are allowed.
- Internal links whose target file does not exist are validation errors.
- Query strings and fragments do not change the resolved page target.

Manual internal and external links are first-class content. No `data-fractal-*` marker is required.

Moving a page updates explicit internal links that resolve to the moved target. Fractal does not infer that arbitrary existing links should change based on their visible text.

## Suggestions

Link suggestions are derived from unlinked visible prose and page titles/filename stems. They are not stored in the format and never mutate documents.

A suggestion contains the matched text and all ranked candidates known to the engine. Ambiguity is returned to the caller rather than treated as invalid state. Applying a suggestion is a separate explicit mutation.

## Validation

Validation checks only invariants required for dependable document operations:

- readable supported manifest;
- existing `pages/` directory;
- identifiable page titles;
- resolvable internal page links.

HTML parsing follows normal HTML5 recovery behavior. Fractal is not an HTML conformance checker; callers that need strict authoring diagnostics should use a dedicated validator.
