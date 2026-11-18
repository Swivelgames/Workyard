# Moving a workspace or state

```sh
workyard relocate --old /work/tasks --new /work/moved
workyard relocate --old /work/tasks --new-state-dir /var/workyard/state
workyard recover --state-dir /work/tasks/.workyard
workyard recover --state-dir /work/tasks/.workyard --undo
```

Move is the default; `--move` is explicit. `--old-tasks-root` and `--new-tasks-root`
are aliases of `--old` and `--new`. Supplying conflicting aliases fails before
movement. Copy/snapshot mode is not implemented.

Omitting the source discovers existing state and asks for source confirmation.
JSON/RPC and closed input require explicit source selection; they never infer consent.
Omitting the destination uses CWD or its discovered workspace root. A state-only
request with `--new-state-dir` keeps the root unchanged. `--old-state-dir` selects
external source state. Colocated `.workyard` follows the root; external state stays
at its current location unless explicitly overridden.

Fresh changed destinations must be absent or empty. Overlapping roots/stores,
occupied targets, conflicting associations and busy gates are rejected. State
aliases share one gate. Workyard never merges workspaces, moves backing clones,
resets branches, overwrites an existing content target or deletes native Git locks.

Workspace content includes loose authored files. Same-volume directory moves keep
the directory intact; cross-volume transfer verifies file bytes before releasing
source copies, preserves hardlinks, symlinks and ordinary permissions, and journals
partial files for safe continuation. Git repairs and verifies linked-worktree
relationships after placement. Participants already moved outside the root stay
there, retain their stored membership names and produce a warning with their path.
Incomplete tasks retain their metadata and unresolved condition. Static unfinished
Git operations do not alone block relocation; actual locks and unconfirmed managed
writers do.

## Authority and interruption

Exclusive gates remain inside their state directories. State transfer excludes
`workspace.lock`, validates paired operation identity and fixed original anchors,
and rebinds typed task, admission, effect, amendment and history placement. Retired
Git locations remain historical facts. Destination activation completes placement;
source cleanup may still need attention.

Plain workspace `recover` always continues toward the original destination.
Before activation, `--undo` always returns to the original source. Neither command
toggles directions. After activation, recovery only cleans positively owned old
metadata; undo is rejected and returning requires a new relocation. Task recovery
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
native-path external-participant warnings and a problem when present. RPC uses
`action: ["relocate"]` with `old`, `new`, `old_state_dir`, `new_state_dir` and optional
`move`; aliases use `old_tasks_root` and `new_tasks_root`. Workspace recovery omits
`task`; an explicitly empty task remains invalid. Task-specific recovery Rust input
and result types remain available.

Relocation upgrades the workspace binding to version 2. Version 1 remains readable;
older binaries reject relocated bindings rather than mutating inactive source state.
Task document versions remain supported. `GitFacts` adds the observed common storage
path, and observation responses add backing paths and optional workspace relocation
facts. Native paths retain lossless encoding.

## Native verification

Linux acceptance exercises real Git, same-volume and `/tmp` to `/dev/shm`
cross-volume movement, partial transfer continuation, hardlinks/symlinks/permissions,
source confirmation through a PTY, JSON/RPC and live Bash/Zsh completion. Tests use
isolated temporary repositories and state, never the live installation.

Windows/macOS/ARM64 still need native gate deletion, no-replace rename, filesystem
identity, permission/symlink behavior, cancellation/process settlement and crash
recovery validation. Compilation or Linux tests do not establish those guarantees
on another platform. Cross-volume copying does not preserve ACLs, extended
attributes or timestamps; ordinary content, links and permissions are the exercised
contract. There is no atomic filesystem/Git/state transaction.
