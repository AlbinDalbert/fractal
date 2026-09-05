# Fractal format contract

Fractal projects contain native Fractal documents stored as HTML. A valid
Fractal project is stricter than a general HTML directory.

## Recognized project entries

Fractal recognizes:

- the root `fractal.json` manifest;
- the `.fractal.lock` coordination file;
- directories below `pages/`;
- native `*.fractal.html` documents;
- `fractal.json` folder metadata below `pages/`.

Every other file is opaque. Fractal does not expose opaque files as pages or
resources and does not list, read, search, validate, hash, rewrite, export, or
report them. Fractal preserves opaque files where they are.

Folder operations that could relocate or delete opaque content use a stricter
rule. Setting a folder title, moving or deleting a folder, and repairing a
folder path scan the complete materialized subtree first. If it contains an
opaque file, symlink, or other unsupported entry, the operation fails before
changing any native or opaque bytes. Deleting a missing ordered folder only
removes its ghost order entry and has no subtree to inspect. Page operations do
not inspect unrelated opaque files in the same folder.

## Project manifest and lock

A project root contains `fractal.json` and a `pages/` directory. Projects
created by Fractal also contain an empty `.fractal.lock` file used to coordinate
processes. Opening and inspection do not create a missing lock. The next
explicit mutation creates it while holding the manifest lock.

The manifest requires these fields:

```json
{
  "name": "Project name",
  "version": 2
}
```

The name must contain a non-whitespace character. The version identifies the
project format, not the crate version or a user revision. Fractal supports
project format 2 only. `Project::open` returns an unsupported-version error for
any other value, while `Project::inspect` reports the unsupported version
without writing files.

Native document paths are UTF-8 paths below `pages/`. Absolute paths and parent
traversal are rejected. A document filename is the kebab-case form of its title
plus `.fractal.html`. A title change renames the document and rewrites stored
native references in the same transaction.

## Folders

Every directory below `pages/`, including `pages/` itself, is a Fractal folder.
A folder may contain a `fractal.json` file:

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

Folder metadata accepts only `title` and `order`. The title must be non-empty.
A nested folder name is the kebab-case form of its title. The pages root is not
renamed. Without metadata, a nested folder uses its directory name as its title
and the pages root uses the project name.

Callers create folders explicitly. `Project::create_folder(parent, title)`
requires an existing parent, derives the directory name from the title, writes
folder metadata, and updates an explicit parent order in one transaction.
`Project::create_page_at` also requires its parent folder to exist. Neither
operation creates missing ancestors.

Only direct child directories and native documents participate in folder
order. Opaque files do not. The default order places child folders first, then
native documents, with case-sensitive Unicode code-point sorting inside each
group.

An explicit `order` is a complete permutation of present children and retained
missing children. Each entry is one direct child name. Entries cannot contain
path separators, parent traversal, duplicates, or `fractal.json`. Directory
names ending in `.fractal.html` are reserved for native documents.

If an explicitly ordered child disappears outside Fractal, its entry remains
as a missing child. Validation reports it and folder inspection returns it with
missing status. Deleting that page or folder through Fractal removes the ghost
entry.

A new child discovered outside Fractal appears after the stored order in the
effective in-memory order. Inspection reports the pending addition. Explicit
project repair appends it to metadata. Fractal-managed child creation, movement,
renaming, and deletion update an existing explicit order in the same
transaction.

Read-only inspection reports native filenames and nested folder names that do
not match their titles. `Project::repair` applies those path changes and pending
order additions. A destination collision fails the repair. Folder path repair
also obeys the opaque-descendant rule.

## Native documents

The `.fractal.html` suffix declares a native document. Fractal catalogs the
file even when its contents are invalid, so inspection and validation can
report the problem.

A valid native document has:

- an HTML doctype;
- `<meta name="fractal-format" content="1">`;
- a non-empty title from `<title>` or the first `<h1>`;
- exactly one `<main data-fractal-document>` as the only body element;
- exactly one direct `<h1 data-fractal-title>` child of that document root;
- exactly one `<style data-fractal-style>` in the head.

The native marker identifies document profile version 1. It is independent of
project format 2.

The native document root accepts these HTML elements:

```text
a abbr b blockquote br caption cite code col colgroup del em
figcaption figure h1 h2 h3 h4 h5 h6 hr i ins kbd li mark ol
p pre q s samp small span strong sub sup table tbody td tfoot
th thead time tr u ul var
```

The head accepts `title`, `meta`, and `style`. Fractal owns the title, charset,
viewport, native marker, managed title heading, and managed style element.
Other `meta` elements and unmarked styles are user-owned. Elements outside this
profile make the native document invalid.

HTML parsing follows HTML5 recovery rules. Parser recovery does not make a
document valid Fractal. `Project::repair_page_structure` can mark or create a
missing managed title and managed style, but it does not remove unsupported
content.

## Native sections and hashes

`Project::native_document_parts` returns the title, content HTML, managed CSS,
and user metadata. It supplies independent SHA-256 hashes for the editable
sections and the exact complete source. `Project::source` returns that source.

Native mutations replace only the requested title, content, managed CSS, or
user metadata. Fractal compares the supplied section hash while holding the
project lock, validates the complete result, and commits it as a recoverable
transaction. This lets edits to different sections merge while a stale edit to
the same section returns `FractalErrorCode::Conflict`.

Live native documents have no whole-source replacement operation.
`NativePageDraft::from_source` parses already-native recovery source, and the
recreation methods use that data only when the destination is still missing.

## Links and search

Stored links are ordinary `<a href="...">` elements. Native link queries index
only relationships between native documents:

- relative links resolve from the source document's folder;
- root-relative links resolve from `pages/`;
- query strings and fragments do not change the resolved target;
- a link to an existing native document is resolved;
- a link that clearly ends in `.fractal.html` but has no target is broken;
- external URLs, fragment-only links, mail links, and local opaque targets are
  ignored by link queries.

Ignored anchors remain in source when Fractal serializes an unrelated section.
Broken native targets fail validation. `Project::insert_link` accepts only a
different existing native target. Moving a native document rewrites affected
references between native documents in the same transaction.

Text search requires every whitespace-separated query term to occur in a native
title or visible native content, without case sensitivity. It reads only the
in-memory native document catalog.

Derived links are case-insensitive exact-title matches in unlinked visible
native text. Fractal reports only titles with one possible target. Results
include DOM text-node ordinals and UTF-16 offsets for rendering. Derivation does
not change source. A stored link is created only by an explicit insertion.

Opening, inspection, validation, search, and link derivation do not write
project files.

## Transactions, receipts, recovery, and repair

Every normal project mutation takes the common lock, refreshes the catalog,
builds a change plan, and commits through the common recoverable transaction
implementation. Page moves, folder moves, backlink rewrites, folder metadata
updates, and document changes commit together when one operation requires them.

Each mutation returns a `MutationReceipt` derived from its plan. Receipts list
created, updated, moved, and deleted project entries. File changes include
SHA-256 hashes when the corresponding bytes are available. Paths are relative
to the project root, use `/` separators, and never identify opaque files.

An incomplete transaction prevents ordinary opening. `Project::inspect`
reports recovery state without changing it. `Project::recover` restores the
pre-operation state. A committed transaction whose cleanup did not finish does
not block opening, but inspection reports it until explicit recovery removes
the transaction directory.

`Project::repair` applies proposed format repairs as explicit operations. A
repair report retains completed changes and a typed failure if a later repair
cannot finish.

## Single-document HTML export

`Project::export_html` validates one native document and writes one standalone
HTML file. It removes Fractal's native marker and document-root attribute while
keeping native content, inline styles, and ordinary anchors.

Direct links to native documents become one-level references in a collapsed
section at the end. A referenced document contributes visible text only, and
its links do not add further references. Derived links can be included by
option and follow the same rule.

## Folder HTML export

`Project::export_folder_html` writes one HTML document from an ordered folder
tree. It walks direct child folders and native documents depth-first in each
folder's effective order. Missing ordered children and opaque files do not
participate.

Without selections, the export includes every present native document below
the requested folder. Selection paths are relative to that folder. A page
selection includes that page. A folder selection includes its full subtree
unless more specific selected descendants narrow it. Selection order does not
change document order. Unknown selections are errors, while selected ghosts
produce no output.

The exporter creates a neutral document shell titled with the effective folder
title. It does not merge source styles. Each included document becomes a
section with a generated `<h1>`, and `<hr>` separates adjacent sections.
Optional numbering prefixes headings after selection, validation, and ghost
filtering.

Links between included native documents become fragment links. Links to
excluded native documents become one-level text references at the end. Derived
links follow the same rules when enabled.

Every included native document must validate. By default, one invalid document
refuses the export. Force mode skips invalid documents and records each path and
reason. An export with no remaining documents still writes a valid empty HTML
document. Export writes only the requested output file and does not mutate the
project.
