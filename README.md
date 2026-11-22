# Workyard

<picture>
	<source media="(prefers-color-scheme: dark)" srcset="assets/workyard-dark.png">
	<img src="assets/workyard-light.png" alt="Workyard">
</picture>

**Work on one feature across several Git repositories.** Give the task a branch name, pick the projects, and Workyard puts a linked worktree for each one in the same task directory. Your main clones stay available for other work.

## Start a task

From a directory where you want to keep your tasks, track two existing Git clones and start a branch across both:

```sh
workyard init
workyard projects track /repos/api /repos/web
workyard create feat/payments api web
```

Workyard uses each clone’s default branch as its starting point. For a local repository without default-branch metadata, supply a starting branch such as `--from main`.

Open `tasks/feat/payments/` in your editor. Its `api/` and `web/` directories are ordinary Git worktrees on `feat/payments`. Commit and test in each repository as usual. Use `workyard checkout` when you want to reuse existing branches.

## Take the task through its lifecycle

| When you want to… | Run |
| --- | --- |
| See your tasks and their participants | `workyard list` |
| Find dirty, missing, or conflicted worktrees | `workyard status feat/payments` |
| Bring another tracked project into the feature | `workyard amend feat/payments infra` |
| Resume an interrupted operation | `workyard recover feat/payments` |
| Retire the task and keep its branches and commits | `workyard remove feat/payments` |

Track a directory of clones once to select projects by name. A new workspace already tracks `projects/`. Use `--workspace` to address the workspace from elsewhere.

## Use it from your tools

Add `--json` for a structured CLI result. Launch `workyard rpc` for local JSON-RPC clients or `workyard mcp` for agents. Both communicate over stdio. Shell completion suggests tasks, tracked projects, and Git references as you type.

See the [command guide](docs/usage.md) for branch selection, removal consent, moving a workspace, recovery, shell setup, and protocol examples.

## Build from source

With Rust and Git installed, clone and build the executable:

```sh
git clone https://github.com/Swivelgames/Workyard.git
cd Workyard
cargo install --locked --path crates/cli
```

## License

You may use Workyard under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).
