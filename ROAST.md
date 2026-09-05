# Codebase roast

Overall, this is a disciplined small engine, not a dead-code dump. The native-only cleanup has landed coherently, the CLI stays thin, and the transaction work has unusually good fault tests. The weak spots are narrower: HTML trust is underspecified, title ownership can split, and committed writes can still be reported as failures.

## Structure and maintainability

- Low. `support.rs` has become a 1,462-line catch-all. It owns export HTML, path normalization, directory scanning, transactions, locking, recovery, hashing, slugs, and search snippets. None of that is dead, but the transaction code is hard to audit while mixed with unrelated helpers. Smallest fix: move the existing transaction and recovery code into one private module and leave the pure path/text helpers where they are. Do not invent a generalized transaction framework.
- Low. The default page template has two sources of truth. `DEFAULT_STYLE` in `project.rs` is duplicated inside the large string in `create_page_at`, followed by `.replace()` calls to add managed attributes. A style change can make new pages differ from repaired pages. Smallest fix: build the template directly from `DEFAULT_STYLE` and write the managed attributes in the literal.
- Note. I found no clear dead production path. Clippy is clean, every direct dependency has an evident owner, and the remaining complexity mostly pays for requirements in the native-only plan.

## Correctness and data integrity

- Medium. A document can have conflicting titles and still validate. `Document::title` prefers any `<title>`, then the first `<h1>`, while validation only counts the managed `<h1>`. It never requires `<title>`, the managed heading, and the title-derived filename to agree. In the no-`<title>` case, an earlier unowned `<h1>` can even make `set_page_title` rename the file while `Page.title` remains unchanged. This also weakens title conflict hashes because the logical section is stored in two nodes but hashed as one string. Smallest fix: define one canonical title rule, validate both owned title nodes against it, and test divergent and missing-`<title>` documents.
- Low. Initialization is not failure-atomic. `Project::init` creates `pages/`, writes the manifest, then creates the lock. A failure late in that sequence leaves a non-empty directory that a retry rejects. Smallest fix: stage initialization in a sibling temporary directory and rename it into place, or clean up only the entries created by the failed call.

## Tests and verification

- Low. CI does not enforce the documented library-only build. `cargo test --no-default-features` passes locally but is absent from `.github/workflows/main.yml`, so CLI feature leakage could reach `main`. Add that one command to the check job.
- Low. The strongest tests mirror the intended transaction path, not the awkward edges above. Fault injection covers interruptions before and around commit, but not a reload failure after a durable commit. There are also no tests for active HTML attributes, dangerous URL schemes, or disagreeing title nodes. Add focused regression tests before changing those contracts.

## Failure handling and operability

- Medium. Durable success can be returned as an ordinary error. Most mutations call `plan.commit(...)` and then propagate `self.reload()?`. If the commit succeeds and reload fails because of an I/O error or an uncooperative filesystem writer, the caller receives an error even though bytes changed. That contradicts the documented durable commit point and invites a blind retry. Smallest fix: represent a post-commit reload failure as an explicit indeterminate or committed-with-warning result, and make the message state that the mutation landed.
- Low. Every mutation reloads and reparses the whole project, while moves and exports parse documents again. There are no file-count or byte-size limits. That is acceptable for small trusted projects, but one large or hostile project can make a simple edit consume unbounded time and memory. Smallest fix: document expected scale first, then add input size limits only if Fractal will open untrusted projects.

## Security and trust boundaries

- Medium when projects can be shared or untrusted. Valid native HTML can still execute active browser behavior. Validation allowlists element names but does not restrict attributes. Event handlers such as `onclick`, `javascript:` links, remote-loading CSS, and permissive user `<meta>` values survive serialization and export. The exporter adds no restrictive CSP. Smallest fix: state that projects are trusted input, or define and enforce an attribute and URL-scheme allowlist for renderable documents and exports.
- Low. The lockfile contains `anyhow 1.0.102`, flagged by `cargo audit` for `RUSTSEC-2026-0190`. It is transitive through target-specific WASI tooling, and this crate does not call the affected API, so the practical risk looks low. `cargo update --dry-run` removes it. Refresh the lockfile and add an audit job that fails on unsoundness warnings.
- Low. CI grants `pages: write` and `id-token: write` to every job. The check job compiles third-party build scripts but needs only repository read access. Put write and OIDC permissions on the deploy job only. Pinning third-party actions to commit SHAs would further narrow supply-chain risk.

## Stale material

- Low. The repository still ships a format-1 `my_test_project`, the completed cleanup plan still says "approved for implementation," and the `contract-v1` tag remains. All three contradict the current format-2-only state. The plan explicitly calls for removing the tag after coordination. Remove the sample, mark or archive the plan, and confirm with collaborators before deleting the shared tag.

## Verification run

- `cargo fmt --all -- --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `cargo test --all-features`: pass, 86 tests including doctests
- `cargo test --no-default-features`: pass, 83 tests including doctests
- `cargo doc --all-features --no-deps`: pass
- `cargo audit`: no vulnerability-class advisory, one unsoundness warning for `anyhow 1.0.102`
