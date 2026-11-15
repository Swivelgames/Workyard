# Workyard working agreements

Before implementation, refactoring or review, read
[Code organization and readability](docs/code-organization-and-readability.md).
Keep these instructions and their examples usable from a standalone Workyard clone.

- Use tabs in formats that permit them; otherwise use four spaces. Do not reindent
	untouched code solely for style.
- Build for growth with focused responsibilities and narrow interfaces. A module
	may stay a small internal component; do not introduce speculative frameworks.
- Keep core lifecycle policy independent of Git/state implementations and frontend
	frameworks. Application owns adapter selection and interface assembly.
- Group cohesive capabilities and let each own its assembly. Compare equivalent
	peers side by side; use consistent vocabulary and stages where meaning matches.
- Keep action contracts separate from views. Compose small renderers that only
	render supplied facts; top-level views retain ordering and completion policy.
- Give each file a primary responsibility. In Rust, small supporting types usually
	precede the primary type and its impls. Extract substantial supporting concepts
	into focused child modules; do not invent types or files merely for symmetry.
- Use meaningful stages, semantic blank lines, and guards that reduce mental
	bookkeeping. Prefer named fields when positional tuples obscure roles.
- Shared behavior belongs to its actual domain. Rehome helpers as responsibilities
	become clear; avoid catch-all common/utilities modules and syntax-only abstractions.
- Preserve public imports, wire/state contracts, native paths and Git authority
	during restructuring. Keep cancellation, lock and publication ordering explicit.
- Preserve intentional user edits. Read complete final changed files and their
	peers after the last fix; formatting and passing tests do not replace this pass.

## Workspace ownership

- `crates/core`: lifecycle operations, domain values and effect contracts.
- `crates/application`: typed actions, input preparation, output contracts and views.
- `crates/git`: native Git commands, inspection and managed process supervision.
- `crates/state`: versioned documents, evidence and workspace/task state access.
- `crates/system`: native identity and stable OS locking.
- `crates/cli`: executable host, interface dispatch and signal cancellation.

Git owns worktree relationships and locations; Workyard owns membership, lifecycle
and evidence. Observation must remain read-only. Preserve successful work and
report damaged/uncertain facts with remedies rather than guessing or resetting.

## Verification

Run checks in this checkout with `CARGO_TARGET_DIR="$PWD/target"`:

```sh
cargo test --workspace --all-targets --locked
cargo test --doc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
git diff --check
```

Use focused regression checks for changed boundaries, then the required workspace
checks. Linux checks do not establish native Windows/macOS behavior. Record actual
platform evidence and remaining checks honestly. Finish with a complete-file
readability pass and compare equivalent capability modules side by side.

## Preserve usability across operations

Recovery is a last resort for actual incomplete effects or unresolved uncertainty.
Refusal, cancellation, validation failure or missing consent must not itself turn
usable state into damaged state. Preserve persisted history and evidence when no
effects occurred; keep invocation outcomes separate from resource condition, and
support ordinary retry. After effects begin, preserve successful work and truthful
pending evidence; never hide damage or discard user work to avoid recovery.
Apply the detailed rule in the code organization and readability guide.
