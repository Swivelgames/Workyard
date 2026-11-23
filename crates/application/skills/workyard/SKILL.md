---
name: workyard
description: Use Workyard to create or check out a task across Git repositories, inspect participants, change task membership, or recover interrupted work.
---

# Manage multi-repository tasks with Workyard

Use Workyard when one task branch needs linked worktrees in one or more Git
repositories. Work in the participant worktrees it creates. Use ordinary Git
inside each participant for changes, commits, and pushes.

## Find the workspace and start work

- Use `workyard info` for workspace and tasks paths, `workyard list` to find task
  names and their participants, and `workyard status <task>` to inspect health
  and Git state.
- Commands discover the nearest workspace from the working directory or take an
  exact `--workspace <directory>`. Use the same workspace across invocations.
  Initialize a new workspace with `workyard init [directory]` when you intend to
  establish one. In an existing workspace, inspect before making changes.
- Use `workyard projects list` to find selectors. The default workspace tracks
  `projects/`. Track additional clones or directories with
  `workyard projects track <path>...`, or pass a repository path with `--repo`.
  When a name is ambiguous, use the qualified selector shown by Workyard.
- Use `create` for a new branch. Use `checkout` when you intend to reuse existing
  branches across the selected repositories. Use `--from` if the starting branch
  needs to be explicit. Put `--repo-from`, `--repo-name`, or `--repo-remote`
  immediately after the `--repo` they apply to. Use `--no-fetch` when working
  offline or intentionally using local references.

```sh
workyard projects list
workyard create feat/payments api web
workyard status feat/payments
workyard amend feat/payments infra
workyard amend feat/payments --remove infra
```

If the task already exists, inspect its status and use the existing worktrees if
the task is the one you intended. If an operation stopped midway, inspect the
named task before retrying. When status shows unfinished work, use
`workyard recover <task>` in the same workspace. Recovery preserves already
created worktrees and can finish the remaining steps. After a filesystem move,
bare `workyard recover` inspects the workspace's Git links. Follow any prompts
about competing copies before choosing an active location.

## Remove worktrees deliberately

Use `workyard amend <task> --remove <participant>` to unlink one participant.
Use `workyard remove <task>` to retire the task and unlink its worktrees.
Removal retains branches and commits. Inspect dirty and unpushed work first.
Use force only when discarding the reported changes is explicitly authorized.

## Choosing an interface

Use `--json` for CLI automation. For an agent with MCP available, start
`workyard mcp` over stdio, discover tools and schemas, and pass the initialized
workspace explicitly if the server runs elsewhere. The MCP `create` tool uses
`checkout: true` for checkout. For local IPC, `workyard rpc` serves JSON-RPC.
Consult the discovered schemas or `workyard <command> --help` before supplying
overrides or attempting a recovery mode.
