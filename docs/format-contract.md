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

## Suggestions and mutations

Link suggestions are derived from unlinked visible text and page titles or filename stems. Suggestions never modify source. Applying one is a separate mutation and is available only for native documents.

Opening, scanning, searching, validating, and suggesting never write files. Native semantic mutations may serialize affected native documents. Raw source changes require an explicit source or filesystem operation.
