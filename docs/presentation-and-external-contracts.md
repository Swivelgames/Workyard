# Presentation and external contracts

Use these principles when designing or reviewing command output, protocol results,
and diagnostics. They complement [Code organization and readability](code-organization-and-readability.md).
The examples illustrate decisions, not mandatory schemas or a fixed UI framework.

## Start with the user's question

Decide what the invocation is for before deciding what fields to print.
Creation answers “what happened during this operation?” Listing answers “what am
I managing?” Status answers “what is its condition now?” Recovery answers “what
was reconciled, what remains unresolved, and what can I do next?”

An operation ending successfully does not mean the managed object is complete.
Creating a task begins its active lifecycle. Keep object lifecycle, operation
outcome, current condition and observation provenance distinct in the model.
Select the relevant facts for each view instead of printing every dimension.

For example, an Active tasks section already establishes membership. A task label
such as needs attention and a missing participant row identify the actionable
exception without repeating active on every line. Modified files can be normal
work; do not present them as task damage merely because they differ from Git HEAD.

Success banners belong to the invocation that succeeded. A list shown days later
must not announce “Creation succeeded” as if the list command performed work.
Relevant past failures may still appear, clearly identified as recorded history.

## Separate internal facts, external contracts and presentation

| Boundary | Owns | Avoid |
| --- | --- | --- |
| Domain and persistence | Lifecycle rules, evidence, progress phases, recovery policy and exact resource identities | Changing facts to make a view convenient |
| Structured external contract | Deliberately exposed typed outcomes, stable problem codes, precise paths and provenance | Serializing internal structs as an accidental public API |
| Human view | Wording, emphasis, hierarchy, path display and actionable guidance | Dumping the contract or teaching implementation mechanics by default |

Internal names need not become external vocabulary. A phase called completed
may mean a worktree creation step finished, not that the task is finished. An
unsettled effect may require human wording such as “a step has an unconfirmed
outcome.” Preserve the underlying distinction; translate its meaning.

Machine consumers need semantic clarity too. Avoid envelopes where status=failed
contains error.status=cancelled and error is actually the full domain outcome.
Distinguish transport/framework failure, operation outcome and presentation failure
through deliberate contracts. Do not casually change established wire shapes while
polishing CLI text: contract changes require their own compatibility decision.

Neither UI strings nor private storage layout should become a machine API. Stable
codes let consumers classify problems; messages and remedies explain their context.
Do not remove structured detail simply because the human view does not print it.

## Be concise without overstating certainty

Present facts at the strength supported by the evidence:

- A successful invocation can report created or recovered when its result supports it.
- Previously recorded completion does not prove that a worktree still exists. On a
	failed recovery, retain wording such as creation recorded for unverified participants.
- A lock establishes contention, not that a particular process is making progress.
- An unconfirmed step does not establish that the process has stopped or that retry
	is safe. Retain the checks or remedies required before recovery.
- Missing, inaccessible and unverified are different conditions. Do not simplify
	them all into missing or healthy.

Avoid repeating generic caveats beneath every task when concise labels or one
shared explanation already establish the distinction. Keep a qualification when
removing it would change what the reader reasonably believes. Concision is not a
reason to hide uncertainty, a destructive consequence or a necessary safety step.

## Make hierarchy carry meaning

Indentation expresses ownership. Workspace errors and empty-state guidance belong
at workspace level; task details belong under a task; participant details must
identify their participant. A shared renderer must not blindly indent every
message as though a task and participant are present.

Use one deliberate blank line between meaningful blocks. Do not make every
component append its own blank line: composition then accumulates whitespace.
Let the containing view own block separation and let row components render rows.
Finish ordinary text output with one newline, not trailing empty paragraphs.

Use spaces in rendered output for predictable alignment. Source-code indentation
is a separate convention. Measure table width from visible text, not ANSI bytes,
and account for Unicode display width. Color supplements words; it cannot carry
meaning alone. Honor terminal capability and color settings independently for
stdout and stderr, and keep protocol output free of terminal decoration.

Workyard's current convention is a two-space inset for Root/State below the title,
four spaces for tasks, and eight for participant tables. Other tools should choose
an intentional hierarchy rather than inherit these exact numbers accidentally.

## Show exceptions instead of predictable repetition

Show a common root once. A branch-shaped task name and participant name usually
supply enough context to locate a conventional workspace. Print the actual path
when it differs, is needed for a remedy, or would otherwise be ambiguous.
Likewise, show a participant branch when it differs from the task name. Preserve
other meaningful exceptions, including detached or unknown state, when available.

Human paths may be relative to a clearly identified root, CWD or home. Structured
paths retain their exact meaning and native representation. Do not abbreviate a
path when doing so conceals which workspace or state directory the command targets.

## Commands are executable instructions, not display strings

A display abbreviation and a shell argument have different requirements. Quoting
“~/state” can prevent home expansion; Rust debug formatting is not shell quoting;
a leading hyphen can be interpreted as another option even when quoted.

Construct command hints from known arguments using a focused shell-aware formatter.
Preserve explicit state selection and the relevant task identity. Use option=value
where needed to keep option-like paths as values, and a positional separator where
the command supports it. Quote for the declared shell; do not claim one command is
portable across POSIX shells, PowerShell and cmd.exe without checking it.

Use a directly usable command when the required values are known. Mark placeholders
as examples when user input is still required. If a native path cannot be represented
faithfully, explain the needed manual input instead of printing a lossy command.
Escape untrusted terminal control characters without corrupting the actual argument.

Prefer a concise condition followed by its next action:

```text
No Workyard state found.
Get started: workyard create --repo /path/to/repo feat/example
```

For a known task and store, a status hint can be concrete:

```text
Inspect: workyard status --state-dir='.workyard state' -- feat/payments
```

Do not replace a required repair or explicit consent with a generic retry command.
An inspection command is useful when diagnosis is needed; it is not proof that
recovery can safely proceed.

## Compose views without moving policy into renderers

Share small components for labels, tables, paths, scoped messages and command
formatting. Top-level views select the content, order, context and completion policy.
Components render supplied facts; they do not inspect Git, discover membership,
mutate state or decide whether recovery is permitted.

```rust
// Illustrative pseudocode: the action reports facts; the view selects meaning.
let outcome = action.execute(input).await;
view.operation_result(outcome.operation);
view.workspace(outcome.workspace);
view.task(outcome.task, condition_label(&outcome));
view.participants(participant_rows(&outcome));
view.problem(outcome.problem, MessageScope::Task);
```

Equivalent commands should use the same components where meaning matches, while
retaining their own policy. Creation and recovery can share table rendering without
sharing all orchestration or pretending they have identical responsibilities.

## Review the output in context

Before delivery, inspect ordinary success, partial failure, missing state, damaged
state and uncertain observations. Compare peer commands and ask:

1. Does this answer the user's question at this point in the object's lifecycle?
2. Could a read-only command sound like it performed a mutation?
3. Is each claim supported by current or explicitly recorded evidence?
4. Does each visible detail help orientation, diagnosis or the next action?
5. Do hierarchy, whitespace and paths reduce repetition without hiding exceptions?
6. Are hints faithful to the intended arguments, shell and native paths?
7. Do plain and styled output preserve the same facts and exit behavior?
8. Have internal terminology and structured contracts remained separate from UI copy?

Use regression checks for meaningful guarantees: no current-availability claims
from stale records, branch exceptions remain visible, shell arguments round-trip,
and errors appear at the right scope. Avoid brittle snapshots of every decorative
character. Record native-platform checks that have not actually been run.
