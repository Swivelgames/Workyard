# Historical relocation experiment

The pre-release relocation command was retired when Workyard adopted explicit
workspace initialization. This page records the previous implementation. For
current commands, see [Start a workspace](../README.md#start-a-workspace).

```sh
workyard relocate --old /work/tasks --new /work/moved
workyard relocate --keep-source --old /work/tasks --new /new/tasks
workyard relocate --old /work/tasks --new-state-dir /var/workyard/state
workyard recover --state-dir /work/tasks/.workyard
workyard recover --state-dir /work/tasks/.workyard --undo
```

Move is the default; `--move` is explicit. `--old-tasks-root` and `--new-tasks-root`
are aliases of `--old` and `--new`. Supplying conflicting aliases fails before
movement. Choose one of `--move` and `--keep-source`.

Omitting the source discovers existing state and asks for source confirmation.
JSON/RPC and closed input require explicit source selection; they never infer consent.
Omitting the destination uses CWD or its discovered workspace root. A state-only
request with `--new-state-dir` keeps the root unchanged. `--old-state-dir` selects
external source state. In move mode, colocated `.workyard` follows the root.
External state stays at its current location unless explicitly overridden.

Fresh changed destinations must be absent or empty. Overlapping roots/stores,
occupied targets, conflicting associations and busy gates are rejected. State
aliases share one gate. Workyard never merges workspaces, moves backing clones,
resets branches or deletes native Git locks. Fresh relocation requires an empty target.

Workspace content includes loose authored files. Same-volume directory moves keep
the directory intact; cross-volume transfer verifies file bytes before releasing
source copies, preserves hardlinks, symlinks and ordinary permissions, and journals
partial files for safe continuation. Git repairs and verifies linked-worktree
relationships after placement. Participants already moved outside the root stay
there, retain their stored membership names and produce a warning with their path.
Incomplete tasks retain their metadata and unresolved condition. Static unfinished
Git operations do not alone block relocation; actual locks and unconfirmed managed
writers do.

## Keep the source files

`--keep-source` requires distinct tasks roots and state directories. Its destination
state defaults to `<new-root>/.workyard`, including when the source uses external
state. Use `--new-state-dir` to choose another destination state directory.

Workyard copies workspace files, verifies them, then repairs the Git registrations
at the destination. The source retains its files and state as an inactive snapshot.
Run Workyard mutations at the active location reported by `workyard info`.
The retained files still contain Git worktree pointers into the shared repository.
Run Git operations from the active worktree. A Git command invoked from the
snapshot can modify the active index or registration.

If interrupted after some registrations have moved, `workyard recover` checks
their current locations with Git and finishes the handoff. Before activation,
`workyard recover --undo` restores the registrations and authority at the source.
The destination remains an inactive partial copy, including any edits made there.
Workyard reports its path so you can inspect the retained files. A fresh copy
requires an empty destination.

### Recovering a participant moved during the copy

If you moved a participant manually and ran `git worktree repair`, run
`workyard recover` from the pending workspace. Workyard locates the participant
through Git. The relocation still determines its destination.

When the destination is vacant and the original and manually moved worktrees
differ, choose **Use files at** for the copy you want. When the destination
already contains a verified copy, choose **Keep files at** to retain its contents
or **Overwrite with files at** to replace them with the registered worktree's
contents. Overwrite removes destination-only files, including ignored and
untracked files. You can back up files in another terminal before answering.
**Cancel** leaves both candidates and the Git registration in place. Equivalent
copies need no decision. After recovery, Git registers the chosen endpoint.
Files at the manually registered path remain available as an ordinary copy.
Use Git from the active endpoint because both directories can contain pointers
to the same administrative storage.

For an unattended conflict, CLI JSON and RPC return `decision_required` with
a `recovery_choice` identifying the participant, destination, whether it is
occupied, and both candidate paths. Rerun interactively to choose.

## Authority and interruption

Exclusive gates remain inside their state directories. State transfer excludes
`workspace.lock`, validates paired operation identity and fixed original anchors,
and rebinds typed task, admission, effect, amendment and history placement. Retired
Git locations remain historical facts. Destination activation completes placement;
source cleanup or snapshot finalization may still need attention.

Plain workspace `recover` always continues toward the original destination.
Before activation, `--undo` always returns to the original source. Neither command
toggles directions. After activation, recovery finishes the move cleanup or marks the copy source
as an inactive snapshot. Returning requires a new relocation. Task recovery
continues to accept the existing task argument and Rust `RecoverInput` API.

Unexpected metadata, changed partial bytes, mismatched identities and uncertain
Git writers stop recovery without overwriting or deleting that content. An owned
pending state directory removed before activation can be recreated. Missing active
state cannot be reconstructed from stale source metadata. Inspect the supplied
problem and preserve both endpoints before correcting any external change.

`info`, `status` and `list` report pending workspace relocation without moving or
implicitly redirecting anything. Ordinary task mutations are blocked until authority
is established. Healthy task conditions follow current Git facts and unresolved
effects, not a historical failed invocation. Human participant tables distinguish
stored membership, actual location and backing storage.

## Interface and format compatibility

CLI `--json` and RPC expose the same relocation outcome: fixed old/new anchors,
operation identity, mutation admission, placement/cleanup completion, active location,
native-path external-participant warnings and a problem when present. Copy results
include `copy: true`. Once settled, `snapshot` identifies the operation, retained
root and state, active root and state, and whether the retained copy is partial.
Inspecting the inactive endpoint with `info`, `list`, or `status` returns those
same snapshot fields. RPC uses
`action: ["relocate"]` with `old`, `new`, `old_state_dir`, `new_state_dir` and optional
`move` or `keep_source` booleans. RPC also accepts the earlier `copy` spelling.
Aliases use `old_tasks_root` and `new_tasks_root`. Workspace recovery omits
`task`; an explicitly empty task remains invalid. Task-specific recovery Rust input
and result types remain available.

Active relocation bindings use version 2. Inactive snapshots and partial copies
use version 3. Workyard reads versions 1 through 3. Older binaries reject the new
snapshot binding before opening it as an ordinary workspace.
Task document versions remain supported. `GitFacts` adds the observed common storage
path, and observation responses add backing paths and optional workspace relocation
facts. Native paths retain lossless encoding.

## Native verification

Linux acceptance exercises real Git, same-volume and `/tmp` to `/dev/shm`
cross-volume movement, partial transfer continuation, hardlinks/symlinks/permissions,
source confirmation through a PTY, JSON/RPC and live Bash/Zsh completion. Tests use
isolated temporary repositories and state, never the live installation.

Move validation on Windows and macOS is complete. Copy/snapshot needs native
checks for permissions, symlink identity, open handles, interrupted repairs and
retained edits on those platforms. Cross-volume copying does not preserve ACLs, extended
attributes or timestamps; ordinary content, links and permissions are the exercised
contract. There is no atomic filesystem/Git/state transaction.
