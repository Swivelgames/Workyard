# Workyard command guide

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

Init installs the bundled agent skill at `.agents/skills/workyard/SKILL.md`.
Re-running init restores a missing skill and preserves any existing edits.
If your agent does not discover that directory automatically, point it at the file.

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

Creation fetches the selected remote by default, then creates each worktree.
Pass `--no-fetch` to use locally available references. A local source branch keeps
its current tip. Explicit `--from origin/main` uses the remote-tracking source.
When the repository has no default branch metadata, supply `--from`.

`create` refuses the operation if the target branch exists in any selected
repository. Use `checkout` to reuse local target branches, track remote-only
targets, and create the branch in the remaining repositories. If the task
already exists, the terminal shows its current status. Source checkouts stay
on their current branches.
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

Ctrl-C stops Workyard from starting the next step. Worktrees that Git has already
created and Workyard has verified stay available. For example, if you interrupt
`amend` after its last addition, the task can already be healthy. If another
participant was still pending, `workyard recover <task>` finishes the outstanding
work. Recovery also checks Git when a state-write failure leaves the outcome uncertain.

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
worktrees. Both whole-task and participant removal disclose files hidden by Git
index flags and files inside uninitialized submodule directories. Empty submodule
placeholders need no discard approval. If Git retained submodule storage after
deinitialization, Workyard identifies the directory to preserve before retrying.
Move any nested Git repository out of the selected worktree before removal.
New changes require a fresh decision. If removal is interrupted after
a worktree changes, run `workyard recover feat/payments` to resume.

New participants use creation's source, remote and fetch options. Reusing an
existing branch requires `--checkout`. Repeating a completed request reports that
no changes are needed. If an addition fails, completed changes such as an earlier
rename survive and you can retry `amend`. To use a leftover branch without verified
creation evidence, pass `--checkout`.

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
```

Recovery opens an initialized workspace, preserves authored changes and reconciles
verified Git moves and repairs. It resumes unfinished worktrees using their recorded
sources. A branch source resolves at its current tip. A commit source retains its
exact commit. A healthy task needs no fetch.

Older tasks may record a request to pull after worktree creation. Recovery ignores
that option. When the writer has settled, Workyard checks the worktree identity
and native Git state before marking setup complete. If Git reports an unfinished
operation, inspect `git -C <path> status`, resolve the operation and retry recovery.

A pending worktree writer, including a pull started by an older Workyard, requires
durable evidence that the writer stopped. A free task lock alone cannot establish
that. Inspect the interrupted Workyard and Git processes and preserve the worktrees
and state. If settlement evidence was lost, restore it from a verified backup.
A pending fetch can be retried under the repository's Git coordination rules.

## Inspection

```sh
workyard list
workyard status
workyard info
workyard status feat/payments --workspace /workspace
```

List shows active tasks, their condition and repository names, followed by up to
five recent closures. Status shows a table of participant names, backing repos
and current worktree states for active tasks. Supply a task name to focus the
report. Backing repository paths are relative to the workspace where possible.
A note identifies any worktree outside its expected path. Status
includes recovery guidance beneath the affected task when applicable.

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
branches and cannot restore discarded files.

## RPC

Run `workyard rpc` and exchange newline-delimited JSON-RPC 2.0 on stdin/stdout.
Pass empty input to initialize the host’s current directory. Wait for the init
response before sending commands that depend on the workspace.

```json
{"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":["init"],"input":{}}}
{"jsonrpc":"2.0","id":2,"method":"invoke","params":{"action":["create"],"input":{"task":"feat/payments","repositories":[{"path":"/repos/api"}],"from":"main"},"observe":true}}
```

Recovery uses the same `invoke` envelope with `action: ["recover"]` and input
`{"task":"feat/payments","workspace":"/workspace"}`.
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

## MCP

Configure an agent client to launch `workyard mcp` over stdio. Clients with an
`mcpServers` configuration can use this entry.

```json
{
	"mcpServers": {
		"workyard": {
			"command": "workyard",
			"args": ["mcp"]
		}
	}
}
```

The host negotiates MCP revision `2025-11-25`. Four tools read workspace state.

| Tool | What it returns | Example arguments |
| --- | --- | --- |
| `list` | Task inventory and participants, including recent closures | `{}` |
| `status` | Current participant health for active tasks or one named task | `{"task":"feat/payments"}` |
| `info` | Workspace paths, discovery source and task counts | `{"workspace":"/path/to/workspace"}` |
| `projects.list` | Available repository references and paths, plus the status of each tracked root | `{}` |

To start in an empty directory, call these tools in order. Wait for each result
before using it in the next call.

| Tool | Example arguments | Result |
| --- | --- | --- |
| `init` | `{}` | Initializes the server’s working directory and returns `workspace` and `tasks_root`. To choose another layout, pass e.g. `{"directory":"/work/payments","tasks":"dev/trees"}`. |
| `projects.track` | `{"paths":["/repos"]}` | Registers one or more repositories or directories to scan. `changed` contains the roots added by this call. |
| `projects.list` | `{}` | Returns the project references you can pass to `create`. |

Use `projects.untrack` with the same `paths` array to stop discovering projects
through those roots. Existing tasks, branches and working files stay available.
Repeating an applied track or untrack returns `{"changed":[]}`.

The writable `create` tool takes a task name and an explicit selection of projects
or repositories. This call uses two references returned by `projects.list`.

```json
{"task":"feat/payments","projects":["api","web"],"no_fetch":true}
```

Sending the call authorizes that creation. Workyard fetches configured remotes
by default. Set `"no_fetch":true` to skip it. Reusing an existing target branch
requires `"checkout":true`.
The terminal result reports the operation outcome and each participant’s phase.

To follow `create` (including checkout), `amend`, `remove`, or named `recover`
while it runs, pass a string or integer `_meta.progressToken` in the
`tools/call` parameters alongside `arguments`.

```json
{"name":"create","arguments":{"task":"feat/payments","projects":["api","web"]},"_meta":{"progressToken":"payments"}}
```

Each `notifications/progress` carries that token, an increasing event count
starting at 1 and a participant message such as `Fetching web`. Git’s remaining
work is unknown, so the host omits `total`. Use a distinct token for each active
call. Clients that omit the token receive only the terminal response.

If the client cancels the call or disconnects, Workyard stops scheduling new
steps and records the outcome of work already in progress.
After a lost reply, call `status` with the same task and workspace. For example,
if `api` exists and `web` is missing after an interrupted create or checkout,
call the writable `recover` tool with `{"task":"feat/payments"}`. This finishes
the outstanding work and preserves edits in `api`. A repeat `recover` reports
`nothing_to_recover: true` once the task is healthy. A repeat `create` returns
`task_exists`.

Use `amend` to add projects or rename an existing participant. For example, this
adds the tracked project `infra` and renames the participant backed by `/repos/api`
to `backend`. Its worktree moves with its uncommitted files.

```json
{"task":"feat/payments","projects":["infra"],"repositories":[{"path":"/repos/api","name":"backend"}]}
```

An observed amendment might report `Renamed participant to backend` followed by
`Fetching infra`. On removal, progress identifies the inspected worktree. The
terminal response carries the file list and any approval the operation needs.

After an interrupted addition or rename, call `status` and then `recover` with
the same task name to finish the outstanding changes. A subscribed recovery can
report `Repairing Git worktree link for api` or confirm that a participant is
unchanged. A healthy task can also return `nothing_to_recover: true` immediately.

Use the CLI or RPC to activate a workspace or undo an interrupted removal.

Use `amend` with `{"task":"feat/payments","remove":["web"]}` to unlink one
participant, or `remove` with `{"task":"feat/payments"}` to retire the whole
task. Both keep the branches and local commits. Clean worktrees can be removed
in one call. A subscribed `remove` reports inspections and successful unlinks,
such as `Unlinked worktree for api`. The successful tool result confirms retirement.

If files would be discarded, Workyard returns `decision_required`. Show the
user each participant’s `disclosures[].dirty_paths`, including ignored files.
With their approval, repeat the same call with the returned `decision.token`
in `approve_discard`. To remove `web` after review, send the following arguments.

```json
{"task":"feat/payments","remove":["web"],"approve_discard":"<token from the reviewed response>"}
```

The client can reconnect before sending approval. If a file changed, review the
new disclosure. If all changes were saved, retry without `approve_discard`.
When some worktrees have already been unlinked, the next decision covers the
outstanding participants. The outcome records the completed changes.

If the removal reply was lost, check `list` for a recent closure and `status`
for the active task. Repeat `remove` only after confirming the same task has an
unfinished removal. A new checkout can reuse a retired task’s name.

After `init`, you can pass its returned `workspace` to each subsequent tool.
Omitting it selects the nearest workspace in the server’s working directory or
its ancestors. Relative paths in `paths` resolve from the server’s working
directory. Results are available in `structuredContent` and as JSON text in `content`.

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
plain displays, and combining JSON mode with `rpc`, `mcp` or `completion` is rejected.
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

Zsh shows named Tasks and Options groups, with tasks first and available help beside
options. Existing Zsh completion styles control the headings. Repository child
options (`--repo-name`, `--repo-remote`, `--repo-from`) are suggested only after a
completed `--repo` value before the cursor; this applies in every supported shell.
The path need not exist, and ordinary command validation is unchanged.
