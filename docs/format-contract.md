# Fractal format contract

A Fractal project may contain native Fractal documents and raw HTML files. Both use HTML as their stored representation, but they have different ownership rules.

## Project

A project contains `fractal.json` and a `pages/` directory. The manifest has two fields:

```json
{
  "name": "Project name",
  "version": 1
}
```

`name` must not be empty. `version` must be supported by the engine. There is no required `.fractal/` directory or persistent graph data.

For read compatibility, Fractal also accepts the former `project_name` field as the project name. Retired manifest fields are ignored. Fractal does not rewrite a legacy manifest when opening it, and legacy `.html` pages remain raw HTML unless an explicit migration converts them to the native document contract.

All page paths are UTF-8 HTML files below `pages/`. Absolute paths, parent traversal, and non-HTML page paths are rejected. Paths identify pages, so duplicate titles are allowed.

## Native documents

Files ending in `.fractal.html` are native documents. Fractal owns their meaning and may normalize their source while applying semantic mutations.

A native document must have:

- an HTML doctype;
- `<meta name="fractal-format" content="1">`;
- a non-empty title from `<title>` or the first `<h1>`;
- exactly one `<main data-fractal-document>` as the only body element.

The native document root accepts the following standard HTML elements:

```text
a abbr b blockquote br caption cite code col colgroup del em
figcaption figure h1 h2 h3 h4 h5 h6 hr i iframe img ins kbd
li mark ol p pre q s samp small span strong sub sup table tbody
td tfoot th thead time tr u ul var
```

This vocabulary covers prose, headings, line breaks, images, lists, inline formatting, quotations, code blocks, figures, and tables. Fractal uses normal HTML meaning for these elements. Attributes remain ordinary HTML attributes.

The document head may contain `title`, `meta`, `link`, and `style`. Scripts and base URL overrides are outside the native profile. Raw HTML and iframe targets may use them.

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

Opening, scanning, searching, validating, and deriving links never write files. Native semantic mutations may serialize affected native documents. Raw source changes require an explicit source or filesystem operation.

## Single-file HTML export

`Project::export_html` accepts one native document and writes one HTML file. It does not export a project or copy a set of files.

The export keeps the source document's HTML content and inline styles, removes the Fractal format marker and document-root attribute, drops external stylesheet links, and replaces images and iframes with `[image]` and `[iframe]` text markers. External links and fragment-only links remain links. Links to native documents become fragment links to their reference blocks. Links to other local files are unwrapped to their content.

Direct links from the source document to other native documents become one-level references in a collapsed `<details>` section at the bottom of the output. Referenced documents contribute their visible text only. Links inside referenced documents do not add more references. Links to raw HTML or other local project files are unwrapped and do not create references. Derived native links can be included with the export option.
