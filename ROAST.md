# Fractal repo review

Reviewed 2026-09-06. Scope: current native-only implementation, with emphasis on
data integrity, read/write reliability, efficiency, and unnecessary complexity.
The findings below drove the reliability and catalog changes in this worktree.

The scope is disciplined, but the reliability claims are stronger than the
implementation earns. Roughly 6,000 lines of runtime Rust and seven direct
dependencies do not make this a dependency landfill. The biggest problems are
recovery edge cases and doing project-sized work for page-sized changes.

## Structure and maintainability

- **Medium: `support.rs` is the junk drawer.** Its 1,503 lines mix transaction
  durability, recovery, path validation, HTML export templates, search snippets,
  and title matching. Move transactions and recovery into a dedicated module so
  auditing rollback does not require scrolling past stylesheet strings. Keep
  the existing concrete operations; no new framework is needed.
- **Low: compatibility remains, but it is not proven dead code.**
  `TransactionPlan::root_relative` supports older transaction paths, and
  `ProjectLock` falls back to locking the manifest when `.fractal.lock` is
  absent. Decide whether those states remain supported before removing the
  branches. Recovery compatibility deserves more caution than ordinary API
  leftovers. See [support.rs](src/project/support.rs).

The library/CLI split is reasonable, dependencies are restrained, and the
native-only cleanup is reflected in the implementation. I found no convincing
pile of dead code. Passing Clippy supports that assessment for private code;
it does not establish that every public API is useful.

## Correctness and data integrity

- **Fixed, high: recovery silently deleted unexpected opaque files.**
  Recovery now removes transaction files individually and removes only empty
  transaction directories. Unexpected content leaves recovery pending and is
  reported as a failure. A regression test adds `keep.txt` after an interrupted
  folder creation and verifies that its bytes survive recovery.
- **Fixed, high: backup durability had a gap.** The commit path now creates and
  syncs the complete backup directory tree before moving originals into it.
  This closes the directory-entry ordering hole found by code inspection. It
  still has not been tested against actual power loss.

The core design is worth keeping: shared/exclusive locks, staged writes,
section-hash conflict checks, commit markers, receipts, and distinct
post-commit errors all address real problems.

## Tests and verification

The following checks passed during this review:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo test --no-default-features`
- `cargo doc --all-features --no-deps`

**Medium: crash-test confidence is narrower than the test names suggest.**
The "actual transaction interruptions" tests inject returned errors at
checkpoints. They exercise interrupted control flow and later recovery, but
do not kill a process, simulate power loss, or interrupt recovery itself.
The unexpected-file case is now covered. Actual process termination, power
loss, and interruption of recovery remain outside the test suite. See
[src/tests.rs](src/tests.rs).

Existing tests cover useful conflict, rollback, opaque-file, and public API
boundaries. The new tests also cover catalog generation swaps and recovery
preservation.

## Failure handling and operability

- **Fixed, medium: read freshness was entirely the caller's problem.**
  `Project::refresh` now reloads under the shared lock, rejects pending
  transactions, and swaps the complete catalog only after every part loads
  successfully. Existing handles remain snapshots until callers refresh them.
  See [storage.rs](src/project/storage.rs) and [lifecycle.rs](src/project/lifecycle.rs).
- **Medium: every small write still pays the whole-project tax.** Mutations
  reload all folders and read, parse, and hash every native document before
  editing, then reload everything afterward. Commit syncs only changed
  directory paths now, but catalog construction still scales with the whole
  project and recovery can walk the full tree. The exclusive lock covers this
  work. Your one-paragraph edit brings the whole filing cabinet to the desk.
  Benchmark writes as projects grow before changing the consistency model. See
  [storage.rs](src/project/storage.rs) and [support.rs](src/project/support.rs).
- **Fixed, medium: repeated queries rebuilt known information.** Search now
  reuses normalized document text. Backlinks and derived links reuse in-memory
  reverse-link and title lookup data. No persistent index was added. There are
  still no benchmarks establishing a comfortable operating range. See
  [search.rs](src/project/search.rs), [links.rs](src/project/links.rs), and
  [storage.rs](src/project/storage.rs).

These are observed cost patterns, not measured latency claims.

## Security and trust boundaries

The implementation checks mutation paths, rejects symlinked ancestors, validates
native HTML, and restricts export destinations. Those are useful boundaries.
This review did not establish that the HTML/CSS filtering is a complete security
boundary against hostile input.

Recovery now treats files added by external tools while Fractal was stopped as
unexpected content and leaves them in place for the operator to handle.

## Priority

The high-risk recovery and durability findings are fixed. The remaining work is
to measure write scaling, reduce whole-project sync and reload costs if the
measurements justify it, and split transaction code out of the miscellaneous
helpers to make it easier to audit. Deleting abstractions is less urgent than
making the existing guarantees true.
