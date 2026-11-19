# Application organization

`crates/application` owns Workyard use cases and interface assembly. Core owns
lifecycle policy; Git and state remain focused adapters. The CLI crate hosts
process arguments, dispatch, signals and terminal setup. It does not own the
application's action implementations or command views.

## Owners and dependency direction

| Owner under `src/` | Responsibility |
| --- | --- |
| `lib.rs` | Deliberate public exports, including existing capability and root paths |
| `application.rs` | Construct the application and register capability entries |
| `actions/<capability>/` | Inputs, preparation, invocation, action outcomes and entry assembly |
| `actions/<capability>/cli/` | Flags/completion bindings, command layout and interface policy |
| `contracts/` | Vocabulary shared by multiple actions and pure core-to-contract conversions |
| `execution/` | Blocking worker boundary, core progress and removal decision adaptation to Trine |
| `workspace/` | Workspace selection and optional read-only reader assembly |
| `observation/` | Read-only orchestration, selection, projections and condition classification |
| `guidance/` | Existing cross-interface human guidance, shell command quoting and safe message text |
| `presentation/structured.rs` | Workyard's structured progress binding defaults |
| `presentation/cli/completion/` | Advisory task, participant and repository value providers |
| `presentation/cli/exit_status.rs` | CLI completion/exit interpretation |
| `presentation/cli/render/` | Supplied-fact components, output buffer, paths/styles and progress rendering |

Actions depend on shared contracts and focused execution/workspace support. Contracts
contain no IO, command registration, styles or execution. Presentation consumes facts;
it does not discover state or mutate it. Observation returns facts and guidance, while
list and status each own their complete layout in their CLI view. Compatibility exports
at the crate root are for consumers; internal imports name the actual shared owner.

Completion is a read-only interface adapter: it may query the workspace reader and Git
name catalog, but never invokes actions, takes mutation admission or runs the heavier
observation pipeline. Ambiguous/incomplete placement options and unavailable state
produce quiet empty suggestions. Candidate names do not promise action success.

## Placement and recovery

Only the init action creates workspace state. Other actions discover an existing
marker in the current directory or its ancestors, or accept a selected workspace.
The optional reader reports missing state without creating directories or locks.
Info exposes the discovery source as part of its diagnostic contract.

Recovery's `action/mod.rs` validates, checks cancellation, resolves state, reads the
record under admission, releases that guard, and routes to removal, amendment or
creation. Local mode modules own invocation options and result interpretation.
They deliberately retain their existing cancellation and no-op semantics. Shared
removal observation belongs to execution because ordinary removal also uses it.

Trine confirmation currently accepts a code and message string. Execution owns the
neutral question wording and preserves the existing escaped message contract; shared
guidance text encoding prevents control characters from reaching that interface.
Terminal disclosure rendering remains in presentation. A richer confirmation protocol
would be a separate API decision. Guidance containing shell-ready commands remains
shared because existing structured outcomes already serialize those strings.

## Extending a capability

For a new action, add its input, handler and outcome in `actions/<name>`. Add preparation
only when normalization merits its own responsibility. Keep action-specific results
local; move a type to contracts only when consumers share its meaning and invariants.
For example, `RepositoryInput` is shared by create and amend; their result types are
still distinct. `Condition` is shared vocabulary; observation classification stays
with observation rather than becoming contract policy.

Build flags and completion registration in the capability's CLI bindings, and put its
layout in its view. Compose shared render components with supplied facts. A table
format change belongs in the shared table component; changing list's sections belongs
in the list view. Progress branching lives in its dedicated component, separate from
the terminal buffer. Keep exit policy at the entry/view boundary.

Have `build_entry` assemble the handler, CLI and structured bindings using the same
stages as its peers, then register it in `application.rs`. The composition root should
not acquire flag names or formatting choices. Recovery modes remain local; do not add
a generic executor, registry, universal outcome or new crate merely for symmetry.

## Compatibility and verification

Physical moves do not remove public Rust paths. Existing root exports, capability
imports, `CreateInput::normalize`, JSON/RPC shapes, state schemas, CLI flags and aliases
remain supported. Keep compatibility re-exports deliberate and internal helpers
private. Removing an import or changing a protocol requires an explicit migration
decision outside an organization-only refactor.

Preserve native paths, cancellation, consent, lock acquisition/release and effect
publication ordering. Run the workspace checks in the README plus
`python3 tests/smoke_dynamic_completion.py` after building the CLI. Public API tests
cover direct imports and distinct outcomes; CLI integration tests cover human/JSON/RPC
behavior, discovery, recovery and completion. Passing Linux checks does not establish
native Windows/macOS or Fish/PowerShell execution.
