mod support;
use std::{fs, path::PathBuf, process::Command};
use support::*;

#[test]
fn repository_routing_environment_cannot_redirect_selected_git_effects() {
	let fixture = tempfile::tempdir().unwrap();
	let selected = repo(fixture.path(), "selected");
	let unrelated = repo(fixture.path(), "unrelated");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let unrelated_before = git(&unrelated, &["show-ref"]);
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.args([
			"create",
			"feat/selected",
			"--repo",
			text(&selected),
			"--from",
			"main",
		])
		.env("GIT_DIR", unrelated.join(".git"))
		.env("GIT_COMMON_DIR", unrelated.join(".git"))
		.env("GIT_WORK_TREE", &unrelated)
		.env("GIT_INDEX_FILE", unrelated.join(".git/index"))
		.env("GIT_OBJECT_DIRECTORY", unrelated.join(".git/objects"))
		.env(
			"GIT_ALTERNATE_OBJECT_DIRECTORIES",
			unrelated.join(".git/objects"),
		)
		.env("GIT_NAMESPACE", "unrelated-context")
		.output()
		.unwrap();
	assert_ok(&output);
	assert_eq!(
		git(&selected, &["rev-parse", "refs/heads/feat/selected"]),
		git(&selected, &["rev-parse", "main"])
	);
	assert_eq!(git(&unrelated, &["show-ref"]), unrelated_before);
	assert_eq!(
		git(&unrelated, &["worktree", "list", "--porcelain"])
			.matches("worktree ")
			.count(),
		1
	);
	assert_eq!(
		git(
			&root.join("tasks/feat/selected/selected"),
			&["rev-parse", "--git-common-dir"]
		),
		text(&selected.join(".git"))
	);
}

#[test]
fn enabled_fetch_resolves_new_selected_remote_source_but_no_fetch_does_not() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	git(&remote, &["branch", "new-source"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let rejected = workyard(
		&root,
		&[
			"create",
			"feat/offline",
			"--repo",
			text(&api),
			"--from",
			"origin/new-source",
			"--no-fetch",
		],
	);
	assert!(!rejected.status.success());
	assert!(!root.join("tasks/feat/offline").exists());
	assert!(git(&api, &["branch", "-r", "--list", "origin/new-source"]).is_empty());

	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/fetched",
			"--repo",
			text(&api),
			"--from",
			"origin/new-source",
		],
	));
	assert_eq!(
		git(&root.join("tasks/feat/fetched/api"), &["rev-parse", "HEAD"]),
		git(&remote, &["rev-parse", "refs/heads/new-source"])
	);
	assert!(!git(&api, &["branch", "-r", "--list", "origin/new-source"]).is_empty());

	let local_missing = workyard(
		&root,
		&[
			"create",
			"feat/local-missing",
			"--repo",
			text(&api),
			"--from",
			"missing-local",
		],
	);
	assert!(!local_missing.status.success());
	assert!(!root.join("tasks/feat/local-missing").exists());
}

#[test]
fn completed_ancestor_task_blocks_descendant_destination_even_without_task_record() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&["create", "feat/one", "--repo", text(&api), "--from", "main"],
	));
	for remove_task_record in [false, true] {
		if remove_task_record {
			let document = fs::read_dir(root.join(".workyard/tasks"))
				.unwrap()
				.next()
				.unwrap()
				.unwrap()
				.path();
			fs::remove_file(document).unwrap();
		}
		let output = workyard(
			&root,
			&[
				"create",
				"feat/one/api/child",
				"--repo",
				text(&web),
				"--from",
				"main",
			],
		);
		assert!(!output.status.success());
		assert!(String::from_utf8_lossy(&output.stdout).contains("path_overlap"));
		assert!(!root.join("tasks/feat/one/api/child").exists());
		assert!(git(&web, &["branch", "--list", "feat/one/api/child"]).is_empty());
	}
}

#[test]
fn no_fetch_uses_cached_remote_source_without_refreshing_it() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	let cached = git(&api, &["rev-parse", "refs/remotes/origin/main"]);
	let upstream = fixture.path().join("api-upstream");
	fs::write(upstream.join("file"), "new upstream commit\n").unwrap();
	git(&upstream, &["commit", "-am", "new remote tip"]);
	git(&upstream, &["push", text(&remote), "main"]);
	assert_ne!(cached, git(&remote, &["rev-parse", "main"]));
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/cached",
			"--repo",
			text(&api),
			"--from",
			"origin/main",
			"--no-fetch",
		],
	));
	assert_eq!(
		git(&root.join("tasks/feat/cached/api"), &["rev-parse", "HEAD"]),
		cached
	);
	assert_eq!(
		git(&api, &["rev-parse", "refs/remotes/origin/main"]),
		cached
	);
}

#[test]
fn per_repository_remote_and_source_override_common_options() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, api_remote) = remote_clone(fixture.path(), "api");
	let (web, web_origin) = remote_clone(fixture.path(), "web");
	let alternate = repo(fixture.path(), "alternate");
	fs::write(alternate.join("file"), "alternate history\n").unwrap();
	git(&alternate, &["commit", "-am", "alternate"]);
	git(&alternate, &["branch", "topic"]);
	let alternate_remote = fixture.path().join("alternate.git");
	git(
		fixture.path(),
		&["clone", "--bare", text(&alternate), text(&alternate_remote)],
	);
	git(
		&web,
		&["remote", "add", "alternate", text(&alternate_remote)],
	);
	git(&api_remote, &["branch", "common-source"]);
	let origin_before = git(&web, &["rev-parse", "refs/remotes/origin/main"]);
	let web_upstream = fixture.path().join("web-upstream");
	fs::write(web_upstream.join("file"), "unselected origin update\n").unwrap();
	git(&web_upstream, &["commit", "-am", "origin update"]);
	git(&web_upstream, &["push", text(&web_origin), "main"]);

	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/overrides",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--repo-remote",
			"alternate",
			"--repo-from",
			"alternate/topic",
			"--remote",
			"origin",
			"--from",
			"origin/common-source",
		],
	));
	assert_eq!(
		git(
			&root.join("tasks/feat/overrides/api"),
			&["rev-parse", "HEAD"]
		),
		git(&api_remote, &["rev-parse", "common-source"])
	);
	assert_eq!(
		git(
			&root.join("tasks/feat/overrides/web"),
			&["rev-parse", "HEAD"]
		),
		git(&alternate_remote, &["rev-parse", "topic"])
	);
	assert_eq!(
		git(&web, &["rev-parse", "refs/remotes/origin/main"]),
		origin_before
	);
	let stored = record(&root.join(".workyard"));
	assert_eq!(stored["participants"][0]["remote"], "origin");
	assert_eq!(stored["participants"][0]["from"], "origin/common-source");
	assert_eq!(stored["participants"][1]["remote"], "alternate");
	assert_eq!(stored["participants"][1]["from"], "alternate/topic");
}

#[derive(Clone, Copy)]
enum MutationDuringProgress {
	ReplaceRepository,
	CreateTargetBranch,
	OccupyDestination,
}

struct MutatingObserver {
	mutation: MutationDuringProgress,
	source: PathBuf,
	tasks_root: PathBuf,
}

impl workyard_core::Observer for MutatingObserver {
	async fn phase(&self, task: &str, _: &str, phase: &str) -> Result<(), workyard_core::Problem> {
		if phase != "creating" {
			return Ok(());
		}
		match self.mutation {
			MutationDuringProgress::ReplaceRepository => {
				fs::rename(&self.source, self.source.with_file_name("original-api")).unwrap();
				repo(self.source.parent().unwrap(), "api");
			}
			MutationDuringProgress::CreateTargetBranch => {
				git(&self.source, &["branch", task]);
			}
			MutationDuringProgress::OccupyDestination => {
				fs::create_dir(self.tasks_root.join(task).join("api")).unwrap();
			}
		}
		Ok(())
	}
}

#[test]
fn creating_progress_cannot_leave_stale_repository_ref_or_destination_validation() {
	use workyard_core::{Control, CreateRequest, OperationOutcome, ProblemCode, RepositoryInput};
	for (mutation, expected) in [
		(
			MutationDuringProgress::ReplaceRepository,
			ProblemCode::RepositoryChanged,
		),
		(
			MutationDuringProgress::CreateTargetBranch,
			ProblemCode::BranchExists,
		),
		(
			MutationDuringProgress::OccupyDestination,
			ProblemCode::TaskExists,
		),
	] {
		let fixture = tempfile::tempdir().unwrap();
		let api = repo(fixture.path(), "api");
		let root = fixture.path().join("workspace");
		init_workspace(&root);
		let root = root.canonicalize().unwrap();
		let state = root.join(".workyard");
		let store = workyard_state::Store::open_existing(&state, None).unwrap();
		let observer = MutatingObserver {
			mutation,
			source: api.clone(),
			tasks_root: root.join("tasks"),
		};
		let request = CreateRequest {
			task: "feat/revalidate".into(),
			repositories: vec![RepositoryInput {
				registration_roots: Vec::new(),
				expected_identity: None,
				path: api.clone(),
				name: None,
				remote: None,
				from: None,
			}],
			tasks_root: root.join("tasks"),
			state_dir: state,
			checkout: false,
			no_fetch: true,
			pull: false,
			remote: None,
			from: Some("main".into()),
		};
		let result = smol::block_on(workyard_core::create(
			request,
			&workyard_git::Runner::default(),
			&Control::default(),
			&observer,
			&store,
		))
		.unwrap();
		let task = result.task;
		assert_eq!(task.operation.outcome, OperationOutcome::Failed);
		assert_eq!(task.problem.unwrap().code, expected);
		assert!(!root.join("tasks/feat/revalidate/api/.git").exists());
		assert_eq!(
			git(&api, &["worktree", "list", "--porcelain"])
				.matches("worktree ")
				.count(),
			1
		);
	}
}
