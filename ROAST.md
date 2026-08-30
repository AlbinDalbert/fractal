# Code roast

## Correctness and data integrity

**High: folder rename rollback is not a rollback.** `rename_folder` renames the directory, then writes rewritten pages and metadata one at a time (`src/project.rs:1750-1822`). If a later write fails, it only renames the directory back. Earlier writes outside the moved folder remain changed, and earlier writes inside it move back with references aimed at the failed destination. This contradicts the documented recoverable transaction guarantee and can leave plausible broken links. Smallest fix: stage the directory move, page rewrites, and metadata changes in the existing recoverable transaction mechanism before changing live files.

**Low: invalid empty project names can be opened and created.** The format contract forbids an empty name, but `Project::init` and `Project::open` do not reject one. The check exists only in project validation (`src/project.rs:1316`). Smallest fix: validate the trimmed name during init and open, with one shared helper.

## Tests and verification

The current checks are clean: formatting, Clippy with warnings denied, and all 56 tests pass.

**Medium: transaction tests miss the dangerous failure window.** Recovery is tested with a hand-built interrupted file transaction, but folder rename has no injected write-failure test. The suite therefore passes while the partial rollback above remains possible. Smallest fix: route rename writes through the existing transaction code, then test recovery from a staged interruption rather than adding a broad mocking layer.

## Failure handling and operability

**Medium: successful transaction cleanup is not durable.** `commit_file_transaction` syncs staged files and the `committed` marker, but does not sync parent directories after renames. A power loss can lose directory entries despite the API having returned success. Smallest fix: sync the affected directories and transaction directory at commit boundaries, or narrow the documentation from crash-safe to process-interruption recovery.
