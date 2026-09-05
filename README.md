# Fractal

Fractal creates, validates, repairs, mutates, searches, links, and exports
native Fractal document projects. Other files may coexist in project
directories, but Fractal does not interpret or manage them.

Fractal is a Rust library with a thin CLI. Native documents use the
`.fractal.html` suffix and a restricted semantic HTML profile. They remain
ordinary HTML files that browsers and HTML tools can inspect, edit, and render.

## CLI quick start

Run the CLI from this repository with `cargo run --`. An installed binary uses
the same arguments without that prefix.

```sh
cargo run -- init ./field-notes --name "Field notes"
cargo run -- --project ./field-notes new-folder "Trips"
cargo run -- --project ./field-notes new "Stockholm" --path trips/stockholm.fractal.html
cargo run -- --project ./field-notes list
cargo run -- --project ./field-notes search "Stockholm"
cargo run -- --project ./field-notes check
cargo run -- --project ./field-notes export-folder-html . --output ./field-notes.html
```

Pass `--json` to any command for machine-readable success or error output.

## Rust API quick start

```rust,no_run
use fractal::{Project, Result};

fn main() -> Result<()> {
    let mut project = Project::init("field-notes", "Field notes")?;
    project.create_folder("", "Trips")?;
    project.create_page_at("trips/stockholm.fractal.html", "Stockholm")?;

    let page = project.page("trips/stockholm")?;
    println!("{}", page.content_hash);

    for result in project.search("Stockholm") {
        println!("{}", result.path);
    }

    Ok(())
}
```

`Project::init` creates a format 2 project. `Project::open` accepts format 2
only. `Project::inspect` reports unsupported versions and recovery requirements
without changing project files.

## Project contents

Fractal recognizes:

- the root `fractal.json` manifest;
- the `.fractal.lock` coordination file;
- directories below `pages/`;
- native `*.fractal.html` documents;
- `fractal.json` folder metadata below `pages/`.

Everything else is opaque. Opaque files do not appear in page lists, search,
links, validation, hashes, receipts, or exports. Fractal leaves them in place.

This matters for folder operations. Changing a folder title, moving or deleting
a folder, and repairing a folder path first check its complete subtree. If the
subtree contains opaque files or unsupported filesystem entries, the operation
fails before changing anything. Create, reorder, and individual page operations
do not fail merely because an unrelated opaque file exists.

The root manifest contains a project name and the only supported project format
version:

```json
{
  "name": "Field notes",
  "version": 2
}
```

Folders must be created explicitly with `Project::create_folder` or
`new-folder`. Page creation, recreation, and movement never create a missing
parent folder. Folder titles and native document titles determine their paths.
Optional folder metadata stores a display title and an explicit order for
direct child folders and native documents.

See [the format contract](docs/format-contract.md) for the native document
profile, folder ordering, links, and exports.

## Catalog, search, and links

Opening a project loads native documents and folders into memory. The catalog
stores each native document's title, visible text, source hash, and native links.
Fractal writes no generated catalog or link-index files.

Text search matches all whitespace-separated query terms against native titles
and visible native content, without case sensitivity. Link queries include only
anchors that resolve to an existing native document or clearly name a missing
`.fractal.html` document. External URLs, fragments, mail links, and local opaque
targets remain in source but do not enter the native link index.

Exact-title derived links find unambiguous, case-insensitive title occurrences
in unlinked visible native text. They are returned with DOM text positions and
never written unless a caller explicitly inserts a selected link.

## Mutations and recovery

Native edits replace one owned section at a time: title, content, managed CSS,
or user metadata. `NativeDocumentParts` supplies a hash for each section, so a
stale edit returns `FractalErrorCode::Conflict`. Fractal reserves whole-source
input for guarded recreation of a missing native document.

Normal mutations take the project lock, refresh the catalog, validate the
candidate state, and use the common recoverable transaction code.
`MutationReceipt` reports created, updated, moved, and deleted entries with
project-relative paths and file hashes where available.

Opening does not perform recovery or repair. An interrupted transaction blocks
`Project::open`. Use `Project::inspect` to see the state, `Project::recover` to
restore pre-operation files, and `Project::repair` to apply title-driven paths
and pending folder-order additions.

## HTML exports

`Project::export_html` writes one standalone HTML document. Links to native
documents become one-level text references at the end of the output. Optional
derived links follow the same rule.

`Project::export_folder_html` writes selected native documents from an ordered
folder tree into one HTML document. Links among included documents become
fragment links. Links to excluded native documents become one-level text
references. Invalid selected documents stop the export unless force mode skips
and reports them.

Export destinations must be outside the project root. Exports write only the
requested output file and do not alter project files.

## Complete CLI command list

Global options are `--project <root>` and `--json`.

```text
fractal init <path> [--name <name>]
fractal --project <root> inspect
fractal --project <root> recover
fractal --project <root> list
fractal --project <root> folders
fractal --project <root> folder <folder>
fractal --project <root> new-folder <title> [--parent <folder>]
fractal --project <root> set-folder-title <folder> <title>
fractal --project <root> set-page-title <page> <title> [--expected-hash <hash>]
fractal --project <root> reorder-folder <folder> <child>...
fractal --project <root> read <page> [--source]
fractal --project <root> parts <page>
fractal --project <root> set-content <page> --file <html-file> --expected-hash <hash>
fractal --project <root> set-style <page> --file <css-file> --expected-hash <hash>
fractal --project <root> restore-style <page> --expected-hash <hash>
fractal --project <root> set-metadata <page> --file <html-file> --expected-hash <hash>
fractal --project <root> repair-page <page>
fractal --project <root> repair-project
fractal --project <root> new <title> [--path <path>]
fractal --project <root> recreate <page> --draft <draft-json>
fractal --project <root> move <page> <destination>
fractal --project <root> move-folder <folder> <destination>
fractal --project <root> delete <page>
fractal --project <root> delete-pages <page>...
fractal --project <root> delete-folder <folder>
fractal --project <root> search <query>
fractal --project <root> links <page>
fractal --project <root> backlinks <page>
fractal --project <root> derived-links <page>
fractal --project <root> link <page> <text> <target>
fractal --project <root> export-html <page> --output <file> [--include-derived-links]
fractal --project <root> export-folder-html <folder> [selection]... --output <file> [--number-sections] [--include-derived-links] [--force]
fractal --project <root> check
fractal help [command]
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features
cargo doc --all-features --no-deps
```

See [the architecture notes](docs/architecture.md) for module responsibilities
and transaction rules.
