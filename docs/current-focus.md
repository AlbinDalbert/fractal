# Fractal Current Focus

This is the short working-memory page for when the project feels too large. If you are not sure what exists, read [`feature-inventory.md`](feature-inventory.md). If the spec makes sense but the code does not, read [`code-map.md`](code-map.md) next.

## What Fractal is

Fractal is the engine for the Fractal project/document format.

It owns:

- the file-backed project layout;
- the stricter-than-HTML page contract;
- validation and repair;
- safe page/note/metadata mutations;
- generated index and graph data;
- search/graph/import/export APIs as engine capabilities.

It is not an end-user desktop UI. Amanite and other clients should consume Fractal through the crate API.

## Current phase

Engine hardening before feature growth.

The next useful work is not choosing a new design pattern. The architecture is already simple enough:

```text
resolve inputs
→ preflight invariants
→ plan mutation
→ apply through MutationPlan
→ rebuild generated data when needed
→ return OperationReport
```

The current phase is making that shape consistent and boring.

## What is already basically done

- Phase 1 format contract baseline.
- Parser-backed validation/repair for the current contract.
- Editor-facing page list/detail and safe update APIs.
- Page create/rename/delete and note mutation APIs.
- Typed operation events/reports and summaries.
- `MutationPlan`, atomic file replacement, and project mutation lock.
- CLI migration toward structured resource/action commands and JSON output.

## Do next

0. Read [`feature-inventory.md`](feature-inventory.md) once to reset what the engine actually supports.

0.5. Build code familiarity by tracing one real flow in [`code-map.md`](code-map.md). Do not refactor during this pass.

1. Audit direct filesystem writes.

   ```sh
   rg -n 'fs::(write|rename|remove_|create_dir|create_dir_all)|File::create|OpenOptions' src
   ```

2. Categorize each hit:
   - okay read-only/support code;
   - mutation internals such as `ops/mutation.rs` or `io/fs.rs`;
   - project operation write that should use `MutationPlan`;
   - intentional exception that needs a comment/test.

3. Move unaccounted user-visible project writes onto `MutationPlan` where practical.

4. Add failure-path tests around any multi-step operation that still has obvious partial-apply risk.

5. Then move to Phase 3: better error codes, structured error context, stable report/JSON surfaces, and rustdoc.

## Do not do yet

- Do not expand graph/search/import/export substantially until the write/report/error surfaces are boring.
- Do not redesign page identity until the current path-first model is documented and tested.
- Do not introduce a service container, command bus, or trait hierarchy without a concrete need.
- Do not let CLI ergonomics define the core library API.

## Done for this phase means

- User-visible project mutations either use `MutationPlan` or have a documented reason not to.
- Generated data is not reported as fresh after failed operations.
- Operation reports are consistent enough for Amanite and scripts to branch on.
- `README.md`, `docs/architecture.md`, `docs/format-contract.md`, `ENGINE-HARDENING-ROADMAP.md`, validation, and tests agree.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.
