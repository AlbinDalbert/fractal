# Native-only v2 cleanup plan

Status: complete

This plan narrows Fractal to one job: operating on projects made of native
Fractal documents. It removes compatibility and file-handling features that
blur that job. The work is intentionally breaking while project format v2 and
the Rust API remain unstable.

Each commit below must leave the crate compiling and its relevant tests
passing. Remove obsolete APIs directly. Do not add deprecation shims or retain
private copies of abandoned behavior.

## Target contract

Fractal recognizes these project entries:

- the root `fractal.json` manifest;
- the `.fractal.lock` coordination file;
- directories below `pages/`;
- native `*.fractal.html` documents;
- folder `fractal.json` metadata.

Every other file is opaque. Fractal does not index, list, read, search,
validate, hash, rewrite, export, or report opaque files. They may coexist with
Fractal entries, but Fractal must not silently delete or relocate them.

The core engine retains:

- project initialization, inspection, validation, recovery, and repair;
- native page creation, recreation, section editing, movement, and deletion;
- explicit folder creation, titles, ordering, movement, and deletion;
- recoverable transactions, conflict detection, and mutation receipts;
- an in-memory native document catalog and link index;
- native text search, links, backlinks, and exact-title derived links;
- explicit insertion of links between native documents;
- concrete single-page and ordered-folder HTML exports.

The current scope does not include import, legacy conversion, raw HTML pages,
assets, embeds, summaries, semantic analysis, ontology, persistent indexes,
mutation history, context packets, or generalized extension and compiler
frameworks.

## Invariants for every commit

- A `.fractal.html` suffix declares a native document. Invalid native source
  remains visible to inspection and validation.
- Other files are neither pages nor assets in the Fractal API.
- Search, stored links, backlinks, and derived links use native documents only.
- Native semantic mutations never replace a whole source file.
- Opening, inspection, validation, search, and link derivation do not write
  project files.
- Normal project mutations use the common lock, change plan, transaction, and
  receipt implementation.
- Unknown files are preserved. A folder operation that cannot preserve them
  must refuse the operation before changing anything.
- HTML export is a concrete output operation, not evidence of a general
  conversion framework.

## Commit 1: record the native-only v2 decision

Suggested commit:

```text
docs: define native-only v2 scope
```

Add this plan and update contributor instructions so new work does not extend
features scheduled for removal.

Remove future-facing requirements for:

- importing or migrating external HTML and Markdown;
- summaries, embeddings, entities, semantic search, and ontology;
- context packets and token budgets;
- persistent graph or index files;
- generalized command, extension, transaction, or compiler frameworks;
- persistent mutation history;
- Amanite-specific implementation work inside the Fractal repository.

Retain the requirements for a library-first API, native search and link
queries, exact-title derived links, reliability, repair, and current HTML
exports.

Do not rewrite the public format contract ahead of the implementation. The
final documentation commit will describe the code that has landed.

Acceptance criteria:

- `AGENTS.md` describes only the approved target contract.
- No contributor instruction treats speculative functionality as required
  current scope.
- This execution plan is the sole roadmap for the cleanup.

## Commit 2: remove project format v1 compatibility

Suggested commit:

```text
refactor: remove v1 project compatibility
```

Remove:

- support for project format version 1;
- the legacy `project_name` manifest alias;
- automatic v1-to-v2 upgrades;
- v1 handling for nested `fractal.json` files;
- v1 fixtures and compatibility tests;
- source comments that promise v1 compatibility.

`Project::open` must accept version 2 only. `Project::inspect` must report an
unsupported version for a version 1 project. Do not add a migration path.

The native document marker may remain
`<meta name="fractal-format" content="1">`. If retained, document it as the
native document profile version, independent of the project format version.

Acceptance criteria:

- Version 2 projects open normally.
- Version 1 projects are inspectable as unsupported and cannot be opened.
- No mutation upgrades an older manifest.
- No v1 fixture or compatibility branch remains.

## Commit 3: make the project catalog native-only

Suggested commit:

```text
refactor: remove raw HTML page support
```

Change project loading and the public page model together so this commit does
not leave unusable raw-page APIs behind.

Remove:

- `PageKind` and `Page.kind`;
- `Project::write_raw_page`;
- `Project::write_raw_page_if_unchanged`;
- `MutationKind::WriteRawPage`;
- the CLI `write` command;
- `.html` fallback and raw-versus-native path ambiguity;
- raw-page movement, deletion, search, link, and backlink branches;
- raw HTML fixtures, helpers, and tests.

Replace the generic HTML collector with a native-document collector that scans
only `*.fractal.html`. Keep `Project::source` and `Project::content_hash` for
native inspection and recovery. Keep `NativePageDraft::from_source`; it parses
already-native recovery source and is not an importer.

Acceptance criteria:

- `pages()`, `page()`, `source()`, `content_hash()`, search, links, and
  validation operate on native documents only.
- An ordinary `.html` file does not appear in any Fractal result.
- An invalid `.fractal.html` file remains visible and fails validation.
- Creating, moving, and deleting native pages retain their current transaction
  guarantees.

## Commit 4: remove assets, embeds, and head links

Suggested commit:

```text
refactor: remove non-native assets and embeds
```

Remove the intentional interpretation of non-native resources.

Remove:

- `Iframe`, `IframeTarget`, and `IframeBacklink`;
- `Project::iframes` and `Project::iframe_backlinks`;
- CLI `iframes` and `embedded-by`;
- iframe parsing, validation, backlink tracking, reference rewriting, and
  deletion guards;
- `LinkTarget::InternalFile` and local non-native file resolution;
- asset enumeration in folder mutation reports;
- `head_links_html` and `head_links_hash` from native parts and drafts;
- `Project::set_page_head_links` and CLI `set-head-links`;
- `<link>`, `<img>`, and `<iframe>` from the native profile;
- export transformations that replace images and iframes with markers or
  remove external stylesheets.

Retain inline managed CSS and user-owned `<meta>` elements. Existing native
documents containing removed elements become invalid and can be edited outside
Fractal before reopening or exporting them. Do not add an automatic rewrite.

Acceptance criteria:

- The public API exports no iframe, asset-target, or head-link types.
- Native validation rejects `<link>`, `<img>`, and `<iframe>`.
- Native drafts and section hashes contain title, content, managed style, user
  metadata, and complete source only.
- No project operation resolves a path to an opaque file.

## Commit 5: restrict link queries to native relationships

Suggested commit:

```text
refactor: restrict links to native documents
```

Make the stored link model describe relationships between native documents,
not every HTML anchor.

- Index links that resolve to existing native documents.
- Represent links that clearly target a missing `.fractal.html` document as
  broken native links.
- Ignore external URLs, fragments, mail links, and local non-native targets in
  project link queries.
- Preserve ignored anchors when an unrelated native section is serialized.
- Restrict `insert_link` targets to existing native documents.
- Rewrite references only between native documents.
- Simplify `LinkTarget` around resolved and broken native targets, or replace it
  with separate resolved-link and validation representations if that produces
  a smaller public API.

Derived links remain a core feature. Both the source catalog and candidate
targets must contain native documents only.

Acceptance criteria:

- `links()` and `backlinks()` report native relationships only.
- External and opaque local anchors remain in document source but do not enter
  the link index.
- Broken native targets fail validation.
- Exact-title derived links keep their existing occurrence and non-writing
  behavior.
- Moving a native document rewrites every affected native reference in one
  transaction.

## Commit 6: name search and indexing precisely

Suggested commit:

```text
refactor: clarify native search and link indexing
```

Keep the existing in-memory catalog, title and visible-text extraction, search,
links, backlinks, and derived links. Do not introduce a public index object.

Rename internal comments and documentation that imply a general graph or
indexing subsystem. Use "native document catalog," "native link index," and
"native text search."

Acceptance criteria:

- Search considers native titles and visible native content only.
- There is no persistent generated state.
- Public documentation does not promise graph traversal, a query language,
  ranking, semantic search, or index lifecycle controls.
- Existing native search and derived-link tests remain intact.

## Commit 7: make folder creation explicit

Suggested commit:

```text
feat: add explicit folder creation
```

Remove implicit parent-directory creation from `create_page_at`. A page's
parent folder must already exist.

Add one narrow operation:

```rust
Project::create_folder(parent, title) -> Result<MutationReceipt>
```

The operation must:

- require an existing parent folder;
- derive the directory name from the title;
- create folder metadata containing the requested title;
- update an explicit parent order when one exists;
- commit directory creation, metadata, and order changes together;
- return all created and updated entries in its receipt.

Add `MutationKind::CreateFolder` and a CLI command:

```text
fractal new-folder <title> [--parent <folder>]
```

Acceptance criteria:

- `create_page_at` returns `NotFound` when its parent folder is absent.
- Creating a folder never creates missing ancestors.
- A successful folder receipt matches the filesystem changes.
- Folder creation participates in fault-injection and recovery tests.

## Commit 8: protect opaque files during folder mutations

Suggested commit:

```text
fix: protect opaque files during folder mutations
```

Before moving, title-renaming, repairing the path of, or deleting a folder,
scan its subtree for entries outside the target contract. If opaque files are
present, refuse the operation before building or committing a mutation plan.

The error should state that the folder contains unsupported content and must be
handled outside Fractal. It does not need to expose the files as project
entries. Ghost-folder deletion does not inspect a missing subtree.

Do not block page operations merely because an unrelated opaque file exists in
the same folder.

Acceptance criteria:

- A failed folder operation leaves native and opaque files byte-for-byte
  unchanged.
- Folder operations still work for subtrees containing only native documents,
  folder metadata, and directories.
- Mutation receipts never contain opaque files.
- Repair records a typed failure rather than moving a folder with opaque
  descendants.

## Commit 9: simplify concrete HTML exports

Suggested commit:

```text
refactor: simplify native HTML exports
```

Keep `export_html` and `export_folder_html`. Remove branches that can no longer
occur after the native-only catalog and profile changes:

- raw-page rejection;
- non-native reference unwrapping;
- image and iframe marker generation;
- external stylesheet removal;
- asset-related report behavior.

Retain:

- single native document export;
- ordered folder export and selections;
- generated section headings and optional numbering;
- links between included native pages;
- one-level text references to excluded native pages;
- optional exact-title derived links;
- validation, force mode, and skipped native page reports.

Acceptance criteria:

- Export code contains no raw-page, asset, iframe, or head-link branches.
- Export reports contain native document paths only.
- Derived links behave the same in page and folder exports.
- Invalid native documents retain the current refusal and force-mode behavior.

## Commit 10: add black-box boundary tests

Suggested commit:

```text
test: enforce native-only public boundaries
```

Keep unit tests beside the engine where fault injection requires private
access. Add integration tests that exercise the public crate and CLI as an
external caller would.

Cover:

- opening and inspecting native-only projects;
- invisibility of ordinary HTML and opaque files;
- rejection of version 1;
- explicit folder creation and missing-parent page creation;
- native-only search, links, backlinks, and derived links;
- stale native section hashes;
- JSON success and error output;
- the final CLI command list;
- removed public behavior through compile-time API shape where practical.

Acceptance criteria:

- CLI errors requested as JSON are serialized as Fractal errors and return a
  nonzero exit status.
- Removed commands do not appear in `fractal --help`.
- The library compiles and tests with `--no-default-features`.
- Integration tests do not depend on crate-private helpers.

## Commit 11: synchronize the public contract

Suggested commit:

```text
docs: document the native-only engine
```

Rewrite the public documentation after the code has reached its final shape.

- Describe Fractal as a project engine for native Fractal documents.
- State that opaque files may coexist but Fractal ignores them.
- Document the folder-mutation refusal rule for opaque descendants.
- Document version 2 as the only supported project format.
- Remove raw HTML, assets, embeds, preview, migration, and v1 language.
- Describe the catalog, search, and links narrowly.
- Document explicit folder creation and the missing-parent rule.
- Update the complete CLI command list.
- Add a minimal Rust API example and CLI quick start.
- Replace or delete the completed reliability plan.
- Delete `ROAST.md`.

The concise product statement should be:

> Fractal creates, validates, repairs, mutates, searches, links, and exports
> native Fractal document projects. Other files may coexist in project
> directories, but Fractal does not interpret or manage them.

Acceptance criteria:

- Every README command exists and every CLI command appears in the README.
- Every documented public operation exists in the Rust API.
- No document describes unimplemented future features as current scope.
- Rustdoc builds without warnings and its example code compiles.

## Repository administration: remove the v1 tag

Removing `contract-v1` is not part of a source commit. Do it only after the
native-only changes have landed and after confirming that no collaborator or
release process still relies on the tag.

Remove both the local tag and its remote reference. Confirm afterward that the
tag is absent locally and on `origin`. This changes a shared Git reference but
does not erase the tagged commit from repository history while another
reference or reflog retains it.

## Verification before final handoff

Run the required project checks:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Also verify the library-only configuration and documentation:

```sh
cargo test --no-default-features
cargo doc --all-features --no-deps
```

Finish with targeted searches for every removed concept. Expected source hits
for raw HTML, `PageKind`, iframe types, head links, `InternalFile`, v1 upgrade,
preview, migration, summaries, semantic tooling, and ontology should be zero,
apart from an explicit historical note if one is intentionally retained.
