# Workyard

Workyard creates a task across explicitly selected Git repositories. The same core
operation is exposed through CLI views and Trine's on-demand stdio RPC host.

This experimental slice implements `create`, its `checkout` alias, `recover`,
`remove`, `list`, `status`, `info`, and `rpc`.
Other lifecycle commands, MCP and TUI are not implemented yet.

```sh
cargo build --bin workyard
workyard create --repo /repos/api --repo /repos/web --from main feat/payments
workyard checkout --repo /repos/api --repo /repos/web feat/existing
```

Each task occupies `<tasks-root>/<branch>/<participant>`. The tasks root defaults
to CWD and state defaults to `<tasks-root>/.workyard`. `--tasks-root` and
`--state-dir` override those paths independently. An explicit state directory
contains the state files directly. Each state directory belongs to one tasks root.

Use `--repo-name`, `--repo-remote` and `--repo-from` immediately after their `--repo`
to override that participant. Common `--remote` and `--from` apply irrespective of
argument position. Repeated aliases of one repository must have compatible names
and options. An existing final task directory or active task record is never adopted.

Creation fetches the selected remote by default, then creates each worktree and
optionally runs plain `git pull` there. `--no-fetch` disables the preliminary fetch;
it cannot be combined with `--pull`. Local default/source branch tips stay local.
Explicit `--from origin/main` uses the remote-tracking source. Missing default
metadata requires an explicit `--from` rather than guessing the current branch.

`checkout` is `create --checkout`: existing local target branches are reused and
remote-only targets become local tracking branches. Source checkouts never switch.
Git's branch/worktree constraints and configuration still apply.

## State and partial results

`.workyard/workspace.json` binds placement. `tasks/<slug>-<sha256>.json` contains
the task's participants and latest outcome; `evidence/` independently preserves
admission intent and pending/settled effect snapshots. Documents use tabs and a
trailing newline. The format is private, versioned and experimental; newer or damaged documents fail without reset.

Stable OS locks protect the workspace, task/prefix resources and Git fetches.
ASCII case aliases share resource locks. Unicode task names conservatively take
an exclusive spelling gate because filesystem equivalence differs by volume;
record identity still hashes the exact task name. Lockfiles remain after release. Do not delete them to bypass contention.
Git common-directory identity and current Git metadata are revalidated before
worktree effects; cached paths are observations rather than a separate Git registry.

Failure or cancellation retains successful worktrees and reports each participant's
last observed phase. Inspect the reported state record and use `git -C <path> status`
and `git -C <source> worktree list --porcelain` to investigate. Do not rerun create on
a partial task: use `recover` with the original task name. A publication error
reports uncertainty, even if the desired JSON is already visible; visibility does not prove durability.

## Recovery

```sh
workyard recover feat/payments --state-dir /workspace/.workyard
workyard recover feat/payments --accept-pull api --accept-pull web
```

Recovery requires existing authoritative state. Explicit `--state-dir` wins;
otherwise the first ancestor `.workyard` of CWD or explicit `--tasks-root` is used.
Recovery preserves completed trees and authored changes, reconciles verified Git
moves/repairs, and resumes only remaining safe work. Healthy tasks do not fetch or
pull for freshness. Branch-valued sources resolve their current tips; explicit
commit sources retain their exact meaning.

An uncertain pull is never automatically replayed. Inspect `git -C <path> status`,
finish or abort the native Git operation manually, then name that participant with
`--accept-pull`. Acceptance preserves your repair and does not bypass unresolved
Git conflicts, native locks, backing/worktree identity checks or uncertain writers.

A pending add/pull without durable writer-settlement evidence remains diagnostic,
even when the task lock is free. Inspect the interrupted Workyard/Git process tree
and preserve the state and worktrees. This slice has no force/unquarantine command:
manual Git work may continue once you have established that writers stopped;
restore only genuinely verified backup evidence if available. Do not edit a
pending marker to manufacture settlement. Legacy completed records can be inspected
at their verified original location; legacy uncertain creating/pulling records
cannot acquire historical settlement through an upgrade. Fetch-only uncertainty
allows explicit recovery retry under the existing native Git coordination rules.

## Inspection

```sh
workyard list
workyard status
workyard info
workyard status feat/payments --state-dir /workspace/.workyard
```

List inspects active tasks and presents a compact inventory: task name, condition
and repository names. It also includes up to five recent closures. Status presents
current participant detail for active tasks, including modified or missing worktrees;
an explicit task selects that active record. Both retain actionable diagnostics for
unresolved state. Neither prints a primary title or workspace metadata.

Use info for workspace root, state directory, discovery source and current record
counts. Explicit state and ancestor `.workyard` are the implemented discovery
sources; the optional user-global locator cache remains deferred. Info reads existing
state only and never initializes a store or mutates Git.

Saved outcomes, independent effect evidence, task-lock contention and current Git
facts remain distinct in structured responses. Lock availability and recorded phases
do not establish that Git writers stopped.

Explicit state takes precedence over ancestor `.workyard` discovery. Missing state
returns no tasks with a create example; existing damage and unknown tasks have
distinct diagnostics. Inspection does not initialize state, fetch, repair worktrees,
refresh the Git index or publish cached locations. Verified moved worktree paths
appear in the response while the saved path remains visible. Listings are
per-record observations, not a global workspace snapshot.

## Removing tasks

```sh
workyard remove feat/payments
workyard remove feat/payments --yes
workyard recover feat/payments --undo
```

Removal discloses participant facts before one initial task-level confirmation.
When files would be discarded, it explains that loss before asking. `--yes` skips
ordinary confirmation but does not authorize file discard. `--force` authorizes
disclosed discard and implies `--yes`. Noninteractive clean removal proceeds;
noninteractive dirty removal without force returns a typed decision requirement.
Fresh destructive facts discovered afterward require renewed consent for that
participant, even with force. Branches and commits are retained.

Declining before effects leaves task history and evidence unchanged and allows
ordinary retry. Partial removal preserves successful work and reports what remains.
Recover can continue an interrupted removal; undo recreates worktrees from retained
branches and cannot restore discarded files. Interrupted publication/recreation
edge cases and native-platform verification remain unfinished in this experimental
slice; a passing ordinary scenario is not a claim of production readiness.

## RPC

Run `workyard rpc` and exchange newline-delimited JSON-RPC 2.0 on stdin/stdout:

```json
{"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":["create"],"input":{"task":"feat/payments","repositories":[{"path":"/repos/api"}],"from":"main"},"observe":true}}
```

Recovery uses the same `invoke` envelope with `action: ["recover"]` and input
`{"task":"feat/payments","accept_pull":["api"],"state_dir":"/workspace/.workyard"}`.
Inspection uses `action: ["list"]`, `["status"]` or `["info"]` with optional `state_dir` and
`tasks_root`; status additionally accepts optional `task`.

RPC uses Trine's shared invocation, progress, cancellation and terminal response
contract. Native Git output never enters protocol stdout. Clean stdin EOF drains
accepted work; shutdown, read failure or output failure requests cancellation.

## Source boundaries and checks

- `core`: create/recovery orchestration and its private preflight, task/request contracts,
	cancellation, problems and narrow effect ports.
- `application`: capability-owned create/recovery input, contract, action and view modules;
	the root registers capabilities and preserves public imports.
- `git`: repository inspection and effects, with private branch planning; the
	process runner and output contract delegate to bounded capture and supervision.
- `state`: the store publishes task records and implements the core port; private
	workspace and admission modules own binding/bootstrap and task guards. Document
	publication, record encoding and path representation remain separate concerns.
- `system`: focused native identity and stable lock modules, reexported at the root.
- `cli`: process entrypoint, signals and CLI/RPC host selection.

Small inseparable concepts stay together: `CommandOutput` owns its diagnostics,
`TaskGuard` owns verification of its locks, and Windows containment keeps its handle
owner beside the managed process. The supervisor visibly allocates drain workers,
launches, waits for the process tree, then joins the drains. Workspace bootstrap
releases exclusive protection before shared binding verification; task publication
still writes independent evidence before the mutable record. Native process behavior
and the standalone prototype stay independent of the application capability layout.

Trine is a sibling path dependency (`../trine`). Builds require both checkouts.
The earlier process-wrap experiment remains `examples/process_prototype.rs`.
Production supervision uses native groups/jobs so cancellation and full managed-tree
completion remain observable independently of the root process's exit.

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --workspace --all-targets
CARGO_TARGET_DIR="$PWD/target" cargo test --workspace --doc
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all --check
CARGO_TARGET_DIR="$PWD/target" cargo clippy --workspace --all-targets -- -D warnings
```

Linux tests use isolated repositories and remotes. Windows cross-compilation checks
are not native validation. Windows console/headless cancellation, Job containment,
case-insensitive filesystems, Unicode paths and macOS process-group completion still
need the tracked native-platform validation pass before production readiness.
