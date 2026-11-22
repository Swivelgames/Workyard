# Developing Workyard

- `core`: create/recovery orchestration and its private preflight, task/request contracts,
	cancellation, problems and narrow effect ports.
- `application`: capability-owned actions and CLI views, with explicit shared contract,
	execution, workspace, observation, guidance and presentation owners. See
	[Application organization](application-organization.md) for dependencies,
	extension examples and compatibility policy.
- `git`: repository inspection and effects, with private branch planning; the
	process runner and output contract delegate to bounded capture and supervision.
- `state`: the store publishes task records and implements the core port; private
	workspace and admission modules own binding/bootstrap and task guards. Document
	publication, record encoding and path representation remain separate concerns.
- `system`: focused native identity and stable lock modules, reexported at the root.
- `cli`: process entrypoint, signals and CLI, RPC and MCP host selection.

Small inseparable concepts stay together: `CommandOutput` owns its diagnostics,
`TaskGuard` owns verification of its locks, and Windows containment keeps its handle
owner beside the managed process. The supervisor visibly allocates drain workers,
launches, waits for the process tree, then joins the drains. Init publishes the workspace marker as a unit. Task publication writes independent
evidence before the mutable record. Native process behavior
and the standalone prototype stay independent of the application capability layout.

From a standalone Workyard checkout, run `cargo build --locked --bin workyard`.
The executable is `target/debug/workyard` (or `workyard.exe` on Windows).

For joint development with an existing Trine checkout, give Cargo its absolute
path for the invocation. This uses local source with the committed lockfile:

```sh
cargo --config 'paths = ["/absolute/path/to/trine"]' build --locked --bin workyard
```

The release build uses Trine 0.0.1 from crates.io. If a local change alters
Trine’s dependencies, use a temporary Cargo patch for crates.io and regenerate
the lockfile locally for the experiment.

The earlier process-wrap experiment remains `examples/process_prototype.rs`.
Production supervision uses native groups/jobs so cancellation and full managed-tree
completion remain observable independently of the root process's exit.

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --workspace --all-targets
CARGO_TARGET_DIR="$PWD/target" cargo test --workspace --doc
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all --check
CARGO_TARGET_DIR="$PWD/target" cargo clippy --workspace --all-targets -- -D warnings
```

Run the official TypeScript MCP SDK smoke test from the Workyard checkout. It
exercises discovery, task operations, progress and reconnect against the release
executable. [SDK smoke test](../tests/smoke_mcp.mjs)

```sh
CARGO_TARGET_DIR="$PWD/target" cargo build --release --locked
npm install --prefix target/mcp-interop --no-save --package-lock=false @modelcontextprotocol/sdk@1.32.2
node tests/smoke_mcp.mjs
```

Linux tests use isolated repositories and remotes. Windows cross-compilation checks
are not native validation. Windows console/headless cancellation, Job containment,
case-insensitive filesystems, Unicode paths and macOS process-group completion still
need the tracked native-platform validation pass before production readiness.
