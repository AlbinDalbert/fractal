# Fractal format contract

A Fractal project may contain native Fractal documents and raw HTML files. Both use HTML as their stored representation, but they have different ownership rules.

## Project

A project contains `fractal.json` and a `pages/` directory. The manifest has two fields:

```json
{
  "name": "Project name",
  "version": 2
}
```

`name` must not be empty. `version` must be supported by the engine. There is no required `.fractal/` directory or persistent graph data.

The manifest version identifies the project format, not the engine package version or a user-managed project revision. Format v1 is stable and corresponds to the repository's `contract-v1` Git tag. Format v2 is the current unstable contract. Changes may accumulate under v2 until a later `contract-v*` tag marks a new stable contract boundary. New projects use v2. The engine can still open v1 projects, but folder metadata is available only in v2. The first folder metadata mutation on a v1 project upgrades its root manifest to v2. A v1 project may contain ordinary assets named `fractal.json` below `pages/`; the engine preserves them and refuses the upgrade until the naming conflict is explicitly resolved.

For read compatibility, Fractal also accepts the former `project_name` field as the project name. Retired manifest fields are ignored. Fractal does not rewrite a legacy manifest when opening it, and legacy `.html` pages remain raw HTML unless an explicit migration converts them to the native document contract.

All page paths are UTF-8 HTML files below `pages/`. Absolute paths, parent traversal, and non-HTML page paths are rejected. A native document's filename is its kebab-case title plus `.fractal.html`. Changing the title renames the file and rewrites stored internal references. Duplicate titles within one folder are therefore rejected by the resulting path collision.

## Folders

Every directory below `pages/`, and `pages/` itself, is a Fractal folder. A folder may contain a `fractal.json` file:

```json
{
  "title": "The Glass Garden",
  "order": [
    "opening.fractal.html",
    "the-crossing.fractal.html",
    "appendix"
  ]
}
```

Folder metadata accepts only `title` and `order`. The title must be a non-empty string. A nested folder's directory name is the kebab-case form of its title. Changing the title renames the directory and rewrites stored internal references. Without metadata, a nested folder's title is its directory name and the pages root's title is the project name. Fractal creates folder metadata when a caller sets a title or submits an explicit order. The pages root is not renamed.

When a project opens, Fractal repairs native filenames and nested directory names that do not match their titles. Opening fails if a repair cannot complete, including when the derived destination already exists. Raw HTML filenames are not title-driven.

Only direct child directories and native `.fractal.html` documents participate in folder order. Raw HTML and other files do not. The default order sorts direct child folders alphabetically first, followed by native documents alphabetically. Sorting is case-sensitive Unicode code-point order and does not use a locale.

An explicit `order` is a complete permutation of the folder's known children. Each entry is one direct child name, with no path separators, absolute paths, parent traversal, duplicates, or `fractal.json`. Directory names ending in `.fractal.html` are reserved because that suffix identifies native documents. A reorder request must contain every present child and every missing ordered child exactly once.

If an explicitly ordered child disappears outside Fractal, its entry remains as a missing child. Validation reports it, and folder inspection returns it with a missing status. A normal page or folder deletion operation removes this ghost entry even though no filesystem object remains.

If Fractal discovers a new direct child while an explicit order exists, it appends that child to the stored order. Fractal-managed creation, movement, renaming, and deletion update affected explicit orders as part of the mutation. Folders without an explicit order remain unordered during those operations.

`fractal.json` is reserved inside every folder. It is metadata, never a page or ordinary asset. Fractal does not follow symlinked directories as folders.

## Native documents

Files ending in `.fractal.html` are native documents. Fractal owns their meaning and may normalize their source while applying semantic mutations.

A native document must have:

- an HTML doctype;
- `<meta name="fractal-format" content="1">`;
- a non-empty title from `<title>` or the first `<h1>`;
- exactly one `<main data-fractal-document>` as the only body element.
- exactly one direct `<h1 data-fractal-title>` child owned by Fractal;
- exactly one `<style data-fractal-style>` in the head.

The native document root accepts the following standard HTML elements:

```text
a abbr b blockquote br caption cite code col colgroup del em
figcaption figure h1 h2 h3 h4 h5 h6 hr i iframe img ins kbd
li mark ol p pre q s samp small span strong sub sup table tbody
td tfoot th thead time tr u ul var
```

This vocabulary covers prose, headings, line breaks, images, lists, inline formatting, quotations, code blocks, figures, and tables. Fractal uses normal HTML meaning for these elements. Attributes remain ordinary HTML attributes.

The document head may contain `title`, `meta`, `link`, and `style`. Scripts and base URL overrides are outside the native profile. Raw HTML and iframe targets may use them. Fractal owns the title, charset, viewport, format marker, marked title heading, and marked style element. The marked style contains arbitrary user CSS and can be restored to the default explicitly. Other styles, user metadata, and head links remain allowed.

Native validation reports unsupported elements, missing structure, broken internal links, and broken local iframe sources. HTML parsing follows HTML5 recovery rules, but recovery does not make a document valid Fractal.

## Raw HTML

Other `.html` files are raw HTML. Their source belongs to the author.

Fractal may read raw source, extract text and links, search it, preview it, and report references to it. Raw files do not need a title or a native document structure. Problems inside raw HTML do not make the Fractal project invalid.

Fractal does not normalize, repair, insert semantic links into, or automatically rewrite raw HTML. A direct source replacement, move, or deletion is explicit. Moving a raw file may update links and iframe sources in native documents that target it, but the raw file itself remains byte-for-byte unchanged.

If a native document moves, Fractal updates relative references inside that native document and references from other native documents. References inside raw HTML remain untouched.

## Links

Links are ordinary `<a href="…">` elements.

- Relative internal links resolve from the source file's directory.
- Root-relative links resolve from `pages/`.
- Fragment-only links remain local fragments.
- URI schemes and protocol-relative URLs are external.
- Relative links to existing non-page files are allowed.
- Query strings and fragments do not change the resolved target.

Native documents must not contain links to missing local targets. Raw files may contain any links, although Fractal still exposes their resolved state to callers.

## Iframes

Native documents may use ordinary `<iframe>` elements. Fractal treats iframes as references distinct from hyperlinks.

- A relative or root-relative `src` may target a project page or another project file.
- A remote `src` is allowed.
- `srcdoc` is allowed and takes precedence over `src`.
- An iframe without a non-empty `src` or a `srcdoc` attribute is invalid in a native document.
- A missing local iframe target is invalid in a native document.

Fractal records `title` and `sandbox` attributes but does not impose permissions. Applications should use sandboxed iframes by default. Remote servers and browser policy may prevent a valid iframe from loading.

Fractal does not interpret an iframe target as part of its containing native document. A raw HTML target remains source-owned.

## Derived links and mutations

Derived links are case-insensitive exact-title matches in unlinked visible text. Fractal reports only matches with one possible target and never stores them in page source. Applications may render these matches as links at runtime. Stored explicit links and derived links remain distinct.

Opening a project may append newly discovered children to an existing explicit folder order. Other scanning, searching, validation, and link derivation do not write files. Native semantic mutations may serialize affected native documents. Native editing replaces only the requested content, managed style, user metadata, or head-link section. Each section has its own SHA-256 comparison hash, so concurrent changes to different sections merge under the project lock. Fractal validates and atomically writes the complete result. Whole-source replacement is available only for raw HTML.

Legacy native documents without the managed title or style markers remain discoverable. An explicit structure repair marks the first direct title heading and first head style when present. It creates a missing managed heading or default style without replacing existing CSS.

## Single-file HTML export

`Project::export_html` accepts one native document and writes one HTML file. It does not export a project or copy a set of files.

The export keeps the source document's HTML content and inline styles, removes the Fractal format marker and document-root attribute, drops external stylesheet links, and replaces images and iframes with `[image]` and `[iframe]` text markers. External links and fragment-only links remain links. Links to native documents become fragment links to their reference blocks. Links to other local files are unwrapped to their content.

Direct links from the source document to other native documents become one-level references in a collapsed `<details>` section at the bottom of the output. Referenced documents contribute their visible text only. Links inside referenced documents do not add more references. Links to raw HTML or other local project files are unwrapped and do not create references. Derived native links can be included with the export option.

## Folder HTML export

Folder HTML export produces one standalone HTML document from an ordered folder tree. It recursively walks direct child folders and native documents depth-first using each folder's effective order. Missing ordered pages and folders are ignored. Raw HTML and other assets do not participate.

Without selections, the export includes every present native document below the requested folder. Selections are page or folder paths relative to that folder. Selecting a page includes exactly that page. Selecting a folder without a more specific selected descendant includes its full subtree. If descendants of a selected folder are also selected, only those selected descendants and their required container path participate. The caller's selection order does not alter document order. Unknown selections are errors; selected ghosts produce no output.

The exporter generates a neutral document shell whose `<title>` is the effective folder title. It does not merge source document styles. Every included page becomes a section identified by its project path. The exporter replaces the source page's first top-level `<h1>` with a generated `<h1>` using the native document title. An `<hr>` separates adjacent page sections. Optional numbering prefixes exported page headings from one after selection, validation, and ghost filtering.

Links between included pages become fragment links to their generated sections. Direct links to excluded native pages become one-level collapsed text references at the end of the combined document. When derived links are enabled, they link to included sections or add one-level references by the same rule. The reference section always follows every exported page and does not recursively collect more references. Links to local non-native files are unwrapped. Images and iframes become `[image]` and `[iframe]` markers.

Every included native document must satisfy the native contract. By default, one invalid document refuses the entire export and identifies its path. Force mode skips invalid documents and records their paths and reasons in the export report. A folder export with no remaining pages still produces a valid empty HTML document.
