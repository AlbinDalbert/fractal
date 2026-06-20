# Fractal Format Contract

This document records the current Fractal project/page contract that the engine is expected to preserve and validation is expected to enforce. It describes **valid Fractal**, not arbitrary valid HTML.

Status: Phase 1 contract baseline. Some edge decisions are intentionally called out at the end so they do not get confused with settled behavior.

## Fractal vs HTML

Fractal pages are HTML-backed so they remain inspectable and renderable with ordinary tools. That does not mean arbitrary HTML is valid Fractal.

Normal Fractal changes should go through the Rust crate API or CLI operations. Raw source access exists as an escape hatch, but candidate HTML must still validate as Fractal before it is saved through `write_page_source`.

## Project Contract

A valid project root contains:

- `fractal.json`
- `.fractal/`
- `.fractal/style.css`
- `pages/`
- the manifest's configured default page

The manifest currently contains:

- `project_name`
- `version`, currently `1`
- `default_page`, normally under `pages/`
- `theme`, currently `dark` or `light`

`.fractal/index.json` and `.fractal/graph.json` are generated data. They are rebuildable cache files, not hand-authored source files. They are schema-versioned and should be regenerated with `fractal index build`, `fractal sync`, or the corresponding library operations.

## Page Document Contract

A valid Fractal page must parse as an HTML document with one effective `<head>` and one effective `<body>`. The generated source includes `<!doctype html>`, `<html>`, `<head>`, and `<body>` explicitly, but current validation is parser-backed and validates the parsed document tree rather than doing literal source-string checks for those tags.

A valid page body contains exactly these meaningful direct children:

1. one `<main>`
2. one `<section data-fractal-notes>`

No other meaningful direct child of `<body>` is valid in the current contract.

The notes section must be outside `<main>` and must be a direct child of `<body>`.

## Head Contract

The page `<head>` must contain:

- exactly one `<title>`
- exactly one `fractal:version` meta tag
- exactly one `fractal:summary` meta tag
- exactly one `fractal:tags` meta tag
- a stylesheet link whose `href` is the exact computed relative path to `.fractal/style.css`

`fractal:version` must match the supported page format version, currently `0.1`.

`fractal:summary` and `fractal:tags` may be empty.

Extra `fractal:*` meta tags are invalid for now. Ordinary non-Fractal meta tags are allowed.

## Title Contract

Every page must have both:

- `<title>... </title>`
- a first meaningful child of `<main>` that is `<h1>... </h1>`

The normalized `<title>` text and normalized first `<main h1>` text must match and must not be empty.

Additional `<h1>` elements inside `<main>` are invalid.

## Body Content Contract

After the required page `<h1>`, `<main>` may contain only this small Phase 1 element subset:

- `p`
- `h2` through `h6`
- `ul`, `ol`, `li`
- `blockquote`
- `pre`, `code`
- generated links: `a[data-fractal-link]`

Arbitrary manual HTML is not valid Fractal input yet. In particular, current validation rejects unsupported elements such as `span`, `div`, `strong`, `em`, and manual `<a href="...">` links.

## Notes Contract

The notes section is mandatory, even when empty.

Every note must be a direct child of the notes section and must have this shape:

```html
<aside id="note-example" data-fractal-note>
  <p>Note body.</p>
</aside>
```

Note IDs must:

- start with `note-`
- contain only lowercase ASCII letters, numbers, and single dashes after the prefix
- not be empty after the prefix
- not start or end with a dash after the prefix
- be unique within the page

Note body content follows the same allowed content subset as normal page body content, without a required heading.

## Link Contract

Fractal currently supports generated links only.

Generated page links must look like:

```html
<a href="other-page.html" data-fractal-link="page">Other Page</a>
```

Generated note links must look like:

```html
<a href="#note-example" data-fractal-link="note">example</a>
```

Validation requires:

- every `<a>` to have `data-fractal-link`
- every generated link to have `href`
- `data-fractal-link="page"` links to resolve to a known project page
- page-link text to identify that target by its page title, case-insensitively
- `data-fractal-link="note"` links to resolve to a note in the same page
- link scopes other than `page` and `note` to be rejected

Manual links are invalid during normal validation. Manual internal page links whose text points at one existing page but names another are reported as label/target mismatches rather than accepted as hidden `href` truth. `repair` may unwrap simple manual links into plain text while preserving their text content. When a generated internal page link points to an existing target but its text does not identify that target, repair keeps the generated link and rewrites its visible text to the target title.

## Repair Contract

`validate` is a read/check operation and should not write project files. It may build derived state in memory to check labels, links, and structure.

`repair` may safely repair Fractal-owned scaffold and markers:

- create missing `.fractal/`
- create missing `.fractal/style.css`
- create missing `pages/`
- create the manifest's missing default page
- add missing required Fractal meta tags
- restore the computed stylesheet link
- restore the manifest theme marker on `<body>`
- add a missing notes section
- merge duplicate notes sections while preserving child content
- restore missing title/heading pairs when one side can be inferred
- unwrap simple manual links into plain text
- rewrite manual or generated internal page links with mismatched text as plain text plus the target title in parentheses

Repairs should not guess at arbitrary user intent or convert arbitrary HTML into Fractal.

## Raw Source Contract

`read_page_source` may return raw HTML for inspection or advanced tools.

`write_page_source` validates the candidate HTML against the project/page contract before saving. If validation fails, the existing page is left untouched. If validation succeeds, the page is saved and generated index/graph data is rebuilt.

## Current Open Edges

These are not blockers for the Phase 1 baseline, but they should be resolved before or during the next hardening passes:

- Decide whether source files must literally contain explicit `<html>`, `<head>`, and `<body>` tags, or whether parser-normalized structure is enough.
- Decide whether additional stylesheet links should remain tolerated or validation should require exactly one Fractal stylesheet link.
- Decide whether a tiny safe inline formatting set such as `strong`, `em`, and `br` should become valid Fractal.
- Tighten list semantics if desired: validation currently focuses on allowed elements and list children, but the contract should eventually be explicit about whether `li` may appear only inside `ul`/`ol`.
- Move all page-writing operations behind the Phase 2 mutation/write layer and validate generated HTML consistently before writes.


## Page titles and slugs

Page titles are the human-facing source of truth for implicit page links. Create APIs accept a title, normalize it to a lowercase kebab-case filename, and store that file under `pages/`. Changing a page title is a rename operation: by default the file path moves to the slug derived from the new title, generated links to that page are repaired, and generated link text is updated to the new title.

Explicit move/import APIs may still accept page paths. Page path components must be lowercase slugs; components may contain lowercase Unicode letters, ASCII digits, single hyphens, and single underscores; `.html` may be omitted and is added automatically. Parent traversal and non-HTML extensions are rejected. Creating new pages whose title labels collide with existing pages is rejected. If duplicate title labels already exist on disk, validation reports them.
