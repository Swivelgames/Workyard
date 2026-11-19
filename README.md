# Workyard

Workyard creates branch-named tasks across Git repositories. The CLI and on-demand
stdio RPC host run the same actions. MCP and the persistent TUI are planned.

## Start a workspace

```sh
mkdir my-workspace
cd my-workspace
workyard init
workyard projects track /repos/api /repos/web
workyard create feat/payments api web
workyard status feat/payments
```

Bare `workyard init` targets the current directory. It creates the fixed state
directory `.workyard/` and the default worktree directory `tasks/`. The example
above puts participants in `tasks/feat/payments/api/` and `tasks/feat/payments/web/`.
The workspace already tracks `projects/`. If your clones live there, you can
skip the track command.

For a workspace with a nested tasks directory, run the following command.

```sh
workyard init my-other-workspace --tasks dev/worktrees
```

The tasks path is relative to the selected workspace. Repeating init on a matching
workspace preserves its state. An explicit conflicting tasks path returns an error.
Task commands find the nearest ancestor workspace or accept `--workspace <directory>`.
Only init creates workspace state. For example, `workyard list --workspace my-other-workspace`
shows tasks in that workspace from outside its directory.

## Selecting projects

`workyard projects list` shows the available references and their registered roots.
Track a single repository or a container of clones. A missing root may be populated
later. Workyard stops at the first Git repository in each subtree, recognizes bare
repositories and linked worktrees, and counts each backing repository once.
Untracking a root preserves task membership and projects reached through another root.

Use the shortest unambiguous suffix, such as `web` or `billing/api`. If two
projects match `api`, the diagnostic and shell completion offer qualified choices.
An explicit path such as `./api` or `/repos/api` selects a filesystem location.
An admitted explicit repository becomes available to future tasks by reference.
When a bare selector also names a directory in the current directory, positionals
use the catalog and `--repo` retains its local directory meaning. Use `./api` to
make a filesystem choice explicit.

```sh
workyard create fix/migration payments/api billing/api
workyard amend fix/migration web --repo infra --repo-from release
workyard projects untrack /repos/web
```

The first command creates flat participant directories named `payments_api` and
`billing_api`. Selecting just one of those projects in a new task gives it the name
`api`. Existing task-local names stay stable. Use `--repo <selector> --repo-name <name>`
to resolve a literal name collision or to choose a descriptive name.

Over stdio RPC, invoke `["projects", "track"]` or `["projects", "untrack"]` with a
`paths` array. Invoke `["projects", "list"]` for roots, coverage and available projects.
Task actions accept a `projects` array of selectors and a `repositories` array of
groups with per-project overrides. For example, `{"task":"feat/payments","projects":["api","web"]}`
is the input for the CLI example above.

## Creating and checking out tasks

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

`.workyard/workspace.json` records the layout. Each record in `.workyard/tasks/`
contains the task’s participants and latest outcome. `.workyard/evidence/` preserves
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

## Amending tasks

```sh
workyard amend feat/payments --repo /repos/worker --from main
workyard amend feat/payments --repo /repos/api --repo-name legacy-api --repo /repos/new-api --repo-name api --from main
workyard amend feat/payments --repo /new/location/api
workyard amend feat/payments --remove infra --remove docs
workyard amend feat/payments --remove api --repo /repos/new-api --repo-name api
```

Amend requires an explicit active task and existing state. It adds repositories,
renames participants through `git worktree move`, and repairs known worktree links
when a supplied moved repository retains its backing identity. Existing participants
keep their names unless `--repo-name` explicitly changes them. Dirty or detached
worktrees retain their files and branch state. Unrelated participants do not need
to be clean. Rename cycles require an intermediate unused name.

Use `--remove` with the participant name shown by `workyard list`. It unlinks the
selected worktree and keeps the backing repository, branch and project registration.
A participant can still be removed after its project is untracked. To retire the
entire task, run `workyard remove feat/payments`.

You can combine additions, renames and removals in one amendment. Workyard checks
the final membership and shows the selected worktrees before asking once. Declining
at this point leaves the task intact. In unattended use, clean worktrees can be
removed with local commits retained on their branches. `--yes` skips the courtesy
prompt. `--force` also authorizes discarding the changes disclosed in the selected
worktrees. Files hidden from Git status by index flags also appear in the disclosure.
A nested Git repository must be moved out of the selected worktree before removal.
New changes require a fresh decision. If removal is interrupted after
a worktree changes, run `workyard recover feat/payments` to resume.

New participants use creation's source, remote, fetch and pull options; existing
branches require `--checkout`. These flags do not pull or reset established
participants. An identical completed request is a notified no-op. Ordinary failures
preserve completed changes and permit `amend` retry; a failed addition does not
roll back a successful rename. A leftover branch without verified creation evidence
still requires explicit `--checkout` rather than silent adoption.

Repository identity comes from Git's backing storage, not its remote URL. Replacement
clones and moves that change filesystem identity are not automatically adopted.
Git performs repairs itself; Workyard never rewrites Git administrative files.
Repairing a moved main clone may reconnect its other linked worktrees too.

Amended membership has an atomic `.revision` document under `evidence/`, anchored
to the original admission. It preserves the latest membership and writer evidence
even if publishing the normal task document is interrupted. Read-only commands use
that verified evidence without writing it. Later mutations use the same authority;
retiring/reusing a task archives the amended membership before a new incarnation.
Participant creation options are versioned separately from the original task-wide
options so recovery preserves each participant's intended execution policy.

## Moving or copying a workspace

Move the workspace directory with your usual filesystem tools, then run
`workyard recover` from its new location. One invocation checks all active tasks.
It repairs verified Git links and respects worktrees you moved outside the workspace.

If you kept the original and both copies share a Git registration, recovery shows
both paths for each affected participant and asks once whether to use this copy.
Both directories retain their working files. Noninteractive clients receive
`decision_required` with the candidate paths. Select this copy explicitly with
`workyard recover --activate-here` or RPC `recover` with `{"activate_here": true}`.
Once the links agree, repeating recovery reports `Nothing to recover`.

If a backing clone also moved, run `workyard projects track /new/path/to/clone`,
then retry recovery. Workyard verifies its Git identity. An unavailable old volume
or a different clone at the expected path leaves that participant unresolved.
Other verified participants can continue. Git can also reconnect other worktrees
of a moved clone. Workyard checks those paths for conflicting owners or active
writers before asking Git to repair them.

## Task recovery

```sh
workyard recover feat/payments --workspace /workspace
workyard recover feat/payments --accept-pull api --accept-pull web
```

Recovery opens an initialized workspace. It preserves completed trees and authored changes, reconciles verified Git
moves/repairs, and resumes only remaining safe work. Healthy tasks do not fetch or
pull for freshness. Branch-valued sources resolve their current tips; explicit
commit sources retain their exact meaning.

An ordinary optional pull failure leaves its successfully created worktree established.
Workyard reports each participant's Git error and exits nonzero, while continuing
other independent participants. Fix or retry the pull with Git; no Workyard acceptance
is needed. Invocation results carry `pull_failures`; status uses current Git facts,
so conflicts remain visible without persisting stale pull errors as task damage.

Legacy or genuinely uncertain pull evidence is never automatically replayed. Inspect `git -C <path> status`,
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
workyard status feat/payments --workspace /workspace
```

List inspects active tasks and presents a compact inventory: task name, condition
and repository names. It also includes up to five recent closures. Status presents
current participant detail for active tasks, including modified or missing worktrees;
an explicit task selects that active record. Both retain actionable diagnostics for
unresolved state. Neither prints a primary title or workspace metadata.

Use info for workspace context, including the tasks directory, state directory,
discovery source and current record counts.

Saved outcomes, independent effect evidence, task-lock contention and current Git
facts remain distinct in structured responses. Lock availability and recorded phases
do not establish that Git writers stopped.

An explicit workspace selects that directory directly. Missing state returns init
guidance. Existing damage and unknown tasks have distinct diagnostics. Inspection
never initializes state, fetches or repairs worktrees. Listings use per-task
observations and can run concurrently with work on other tasks.

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

Run `workyard rpc` and exchange newline-delimited JSON-RPC 2.0 on stdin/stdout.
Pass empty input to initialize the host’s current directory. Wait for the init
response before sending commands that depend on the workspace.

```json
{"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":["init"],"input":{}}}
{"jsonrpc":"2.0","id":2,"method":"invoke","params":{"action":["create"],"input":{"task":"feat/payments","repositories":[{"path":"/repos/api"}],"from":"main"},"observe":true}}
```

Recovery uses the same `invoke` envelope with `action: ["recover"]` and input
`{"task":"feat/payments","accept_pull":["api"],"workspace":"/workspace"}`.
Amend uses `action: ["amend"]` and the same repository groups/options as creation,
with an explicit task and existing-state selection. For example,
`{"task":"feat/payments","remove":["infra","docs"],"yes":true}` removes two
participants. Its result separates operation
outcome, retained participant facts and per-participant changes.
Inspection uses `action: ["list"]`, `["status"]` or `["info"]` with optional `workspace`.
Status also accepts an explicit `task`.

RPC uses Trine's shared invocation, progress, cancellation and terminal response
contract. Native Git output never enters protocol stdout. Clean stdin EOF drains
accepted work; shutdown, read failure or output failure requests cancellation.

## Source boundaries and checks

- `core`: create/recovery orchestration and its private preflight, task/request contracts,
	cancellation, problems and narrow effect ports.
- `application`: capability-owned actions and CLI views, with explicit shared contract,
	execution, workspace, observation, guidance and presentation owners. See
	[Application organization](docs/application-organization.md) for dependencies,
	extension examples and compatibility policy.
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
launches, waits for the process tree, then joins the drains. Init publishes the workspace marker as a unit. Task publication writes independent
evidence before the mutable record. Native process behavior
and the standalone prototype stay independent of the application capability layout.

Trine is a sibling path dependency (`../trine`). With both checkouts in place,
build the CLI from the Workyard checkout using `cargo build --bin workyard`.
The executable is `target/debug/workyard` (or `workyard.exe` on Windows).
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

## JSON output and shell completions

Add `--json` before or after any action to receive one structured result:

```sh
workyard --json list
workyard status feat/payments --json
workyard create --repo /repos/api --from main feat/payments --json
```

The document contains `exit_code`, `result`, and `error`, with four-space indentation
and a trailing newline. The process exit code agrees with `exit_code` when output
is delivered successfully. `result` contains the same typed action data available
over RPC, including domain problems and partial/cancelled outcomes. `error` is null
unless CLI/framework execution or presentation failed; it then contains a stable
`code` and readable `message`. There is no JSON-RPC wrapper.

JSON mode suppresses progress, styling and prompts. It does not imply `--yes` or
`--force`; existing noninteractive consent rules apply. Explicit help/version remain
plain displays, and combining JSON mode with `rpc` or `completion` is rejected.
A failed output stream can prevent delivery of a complete document; check the exit
code and stderr too.

Generate a completion script for Bash, Zsh, Fish or PowerShell:

```sh
workyard completion bash > workyard.bash
workyard completion zsh > _workyard
workyard completion fish > workyard.fish
workyard completion powershell > workyard.ps1
```

Load or install the script using your shell's normal completion configuration.
Workyard never edits that configuration. Reload or regenerate scripts after upgrading;
completion uses the installed executable to query current data.

`amend`, `remove`, `status`, and `recover` suggest recorded non-retired task names.
Damaged or busy tasks can still appear; a suggestion is not a promise that an
operation will succeed. Completion uses the same workspace selection as the action.
Recorded-task completion only reads existing state. Unavailable context returns
no suggestions. `create` and `checkout` task names remain free-form, without file
suggestions. Path options still complete files and directories.

Repository remotes and local references are suggested from their own `--repo` group.
Common `--remote`/`--from` suggestions intersect explicitly supplied repositories,
excluding the corresponding per-repository overrides. References include local branches,
cached remote-tracking branches and tags; no network query or fetch occurs.

`checkout` and `create --checkout` suggest the union of existing local branches and
branches cached for each repository's selected remote. New names remain valid.
`recover --accept-pull` suggests only participants with recorded unsettled pulls in the
selected task. These are advisory names, not evidence that recovery is currently safe.

Zsh shows named Tasks and Options groups, with tasks first and available help beside
options. Existing Zsh completion styles control the headings. Repository child
options (`--repo-name`, `--repo-remote`, `--repo-from`) are suggested only after a
completed `--repo` value before the cursor; this applies in every supported shell.
The path need not exist, and ordinary command validation is unchanged.

Bash and Zsh have live local verification. Fish and PowerShell scripts and Rust
transport are covered by tests, but native execution remains a platform acceptance
item. Unresolved shell expressions are not evaluated during completion.
