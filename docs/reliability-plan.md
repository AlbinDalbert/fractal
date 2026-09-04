# Reliability plan

This plan records the reliability work for Fractal format v2. Directory projects remain the canonical editable representation. ZIP packaging, general import and export work, and Amanite UI work are outside this plan.

## Decisions

- Fractal may make breaking Rust API changes while format v2 is unstable. Amanite should update its pinned revision after this work lands.
- Opening a project must not change project files.
- Transaction recovery and format repair are explicit operations. Each returns a report of every change it made.
- Public reports use validated UTF-8 paths relative to the project root. Examples are `fractal.json` and `pages/index.fractal.html`.
- Fractal returns operation receipts but does not keep a permanent mutation log.
- A successful mutation means the new state reached its durable commit point. Cleanup trouble after that point is a warning, not a failed mutation.
- Multi-step recovery and repair reports preserve completed changes and include typed failures if a later step cannot finish.
- Recreating a missing page never overwrites a path that has reappeared.
- All project mutations use the same transaction and reporting machinery. Export output files are not project mutations.
- New projects contain an empty `.fractal.lock` coordination file. Existing projects acquire it on their first explicit mutation; inspection and opening alone do not create it.

## Work sequence

### 1. Transaction guarantees

- Replace the pages-only transaction plan with a project-relative plan that can include the root manifest.
- Use a stable project lock whose identity does not change when `fractal.json` is replaced.
- Route single-file project mutations through the same transaction code as multi-file mutations.
- Commit folder title metadata, directory movement, backlink rewrites, and manifest upgrades as one operation.
- Sync transaction data and affected directories at the required durability boundaries.
- Treat a persisted commit marker as success even when transaction-directory cleanup fails.
- Return a typed error when commit or rollback leaves the outcome indeterminate.

Acceptance criteria:

- `Ok` is returned only after the requested project state has committed.
- An error before commit leaves the old state restored unless the error explicitly reports an indeterminate outcome.
- A cleanup failure after commit cannot be reported as a failed mutation.
- No project mutation uses the old standalone atomic-write path.

### 2. Change planning and receipts

- Introduce one internal change plan used to execute mutations and construct reports.
- Replace the current `Mutation` path lists with a typed `MutationReceipt`.
- Report created, updated, moved, and deleted files with direct move mappings.
- Include before and after SHA-256 hashes for regular files when their bytes are available.
- Report no-op operations and non-fatal warnings.
- Include indirect changes such as rewritten backlinks, folder metadata, and the root manifest.

Acceptance criteria:

- Receipt tests compare the report with the files that changed on disk.
- Callers never need to infer a move by pairing two unrelated lists.
- Serialized receipts contain only slash-separated project-relative paths.

### 3. Read-only inspection and explicit recovery

- Make `Project::open` read-only.
- Add path-based inspection that works before a project can be opened normally.
- Report pending transactions, malformed recovery state, title and path mismatches, folder-order additions, validation problems, and unsupported versions.
- Add explicit transaction recovery and format repair operations.
- Reuse the common file-change representation in recovery and repair reports.

Acceptance criteria:

- A byte-for-byte snapshot of a project is unchanged after inspection and ordinary opening.
- Pending interrupted transactions prevent normal opening but remain inspectable.
- Recovery and repair reports list every file they restore, remove, create, update, or move.

### 4. Guarded page recreation

- Add a native draft value containing the title, content, managed style, user metadata, and head links.
- Recreate the complete native document in one locked transaction.
- Validate the result and its title-derived path before writing.
- Update explicit folder order in the same transaction.
- Return `Conflict` if the destination exists when the operation owns the lock.

Acceptance criteria:

- Recreation cannot overwrite a page created by another process.
- A failed recreation leaves no partial page.
- A successful receipt includes the page and any changed folder metadata.

### 5. Health reporting

- Build a serializable health report from inspection and validation.
- Distinguish errors that block opening, repairs that require approval, ordinary validation issues, and recovery cleanup warnings.
- Keep recent mutation history in callers such as Amanite rather than adding persistent generated state to a Fractal project.

### 6. Reliability tests and fixtures

- Add permanent v1, v2, invalid, and repairable project fixtures.
- Add fault injection around every transaction phase.
- Test that reopening produces the complete old state or the complete committed state, never a mixture.
- Test receipt completeness and read-only opening.
- Expand CI to supported desktop operating systems when Amanite's platform policy is settled.

### 7. Documentation and Amanite handoff

- Update the README, architecture, and format contract with the implemented API and behavior.
- Record breaking API changes and the Amanite migration sequence.
- Update Amanite's pinned Fractal revision only after these acceptance criteria pass.

## Implementation status

The Fractal work in sections 1 through 6 is implemented and covered by the checked-in fixtures and tests. The Fractal documentation portion of section 7 is complete. Updating Amanite's dependency and consuming these breaking APIs remains an Amanite-side follow-up, and desktop CI expansion remains intentionally pending a supported-platform decision.

## Amanite migration sequence

1. Update Amanite's pinned Fractal revision and replace uses of the former `Mutation` return value with `MutationReceipt`.
2. Keep recent receipts in Amanite application state if the UI needs mutation history; Fractal does not persist a log.
3. Inspect a selected project before opening it. If inspection reports pending recovery, show the report and invoke `Project::recover` only after the user chooses recovery. Re-inspect before opening.
4. Present `proposed_repairs` separately from validation issues. Invoke `Project::repair` only after approval, and display its completed changes, warnings, and failures.
5. When a saved page disappears, offer recreation from the editor-owned durable draft. Amanite's existing complete native source can use `Project::recreate_page_from_source`; structured draft storage can use `Project::recreate_page_from_draft`. Treat `Conflict` as a newly reappeared file that must not be overwritten.
6. Build project health from `ProjectInspection`, then combine it with Amanite-owned draft and save state. Do not infer filesystem changes from paths or operation names when the receipt already provides exact mappings and hashes.
7. Add packaged application tests for recovery, repair approval, receipt display, recreation conflict, and stale-hash save conflicts before treating the new revision as the default.

## Deferred work

- ZIP packaging for snapshots, backup, or sharing.
- New importers, exporters, and conversion loss reports.
- Persistent mutation history.
- Amanite recovery-draft storage, UI, allocator investigation, CSP, and API-key storage.
