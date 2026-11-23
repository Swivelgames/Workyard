//! Linux-only create/recover experiment. The Git supervisor and the receipt
//! consumer are confined to this fixture; the CLI under test is the normal binary.
#![cfg(target_os = "linux")]

#[allow(dead_code)]
mod support;

use std::{
	env, fs, io,
	os::unix::fs::{MetadataExt, PermissionsExt},
	path::{Path, PathBuf},
	process::{Child, Command, Output, Stdio},
	thread,
	time::{Duration, Instant},
};
use support::{assert_ok, git, repo, text};
use workyard_core::{EffectState, OperationKind, Phase, RecoveryState, State};
use workyard_state::{Store, task_filename};

const TASK: &str = "feat/test";
const SUPERVISOR: &[u8] = include_bytes!("support/writer_exit/supervisor.py");
const HOOK: &[u8] = include_bytes!("support/writer_exit/hook.py");

struct Fixture {
	dir: tempfile::TempDir,
	root: PathBuf,
	path: std::ffi::OsString,
	real_git: PathBuf,
	repository: PathBuf,
}

impl Fixture {
	fn new() -> Self {
		let dir = tempfile::tempdir().unwrap();
		let base = dir.path();
		let root = base.join("workspace");
		let bin = base.join("bin");
		support::init_workspace(&root);
		fs::create_dir(&bin).unwrap();
		let repository = repo(base, "api");
		executable(&bin.join("git"), SUPERVISOR);
		executable(&repository.join(".git/hooks/post-checkout"), HOOK);

		let original_path = env::var_os("PATH").unwrap();
		let real_git = env::split_paths(&original_path)
			.map(|dir| dir.join("git"))
			.find(|git| git.is_file())
			.unwrap()
			.canonicalize()
			.unwrap();
		let path =
			env::join_paths(std::iter::once(bin).chain(env::split_paths(&original_path))).unwrap();
		Self {
			dir,
			root,
			path,
			real_git,
			repository,
		}
	}

	fn state(&self) -> PathBuf {
		self.root.join(".workyard")
	}

	fn effects(&self) -> PathBuf {
		self.state()
			.join("evidence")
			.join(task_filename(TASK))
			.with_extension("effects")
	}

	fn command(&self) -> Command {
		let mut cmd = Command::new(env!("CARGO_BIN_EXE_workyard"));
		cmd.current_dir(&self.root)
			.env("GIT_CONFIG_NOSYSTEM", "1")
			.env("GIT_CONFIG_GLOBAL", "/dev/null")
			.env("WY_FIXTURE", self.dir.path())
			.env("WY_REAL_GIT", &self.real_git)
			.env("WY_PENDING", self.effects())
			.env("PATH", &self.path);
		cmd
	}

	fn start(&self, case: &str) -> Child {
		self.command()
			.env("WY_CASE", case)
			.args([
				"create",
				TASK,
				"--repo",
				text(&self.repository),
				"--from",
				"main",
				"--no-fetch",
			])
			.stdout(Stdio::null())
			.spawn()
			.unwrap()
	}

	fn wait(&self, marker: &str) {
		let deadline = Instant::now() + Duration::from_secs(15);
		while !self.dir.path().join(marker).exists() {
			assert!(Instant::now() < deadline, "timed out waiting for {marker}");
			thread::sleep(Duration::from_millis(10));
		}
	}

	fn release(&self, marker: &str) {
		fs::write(self.dir.path().join(marker), []).unwrap();
	}

	fn crash_at(&self, case: &str, marker: &str) {
		let mut workyard = self.start(case);
		self.wait(marker);
		workyard.kill().unwrap();
		workyard.wait().unwrap();
	}

	fn recover(&self) -> Output {
		apply_success_receipt(&self.state(), &self.dir.path().join("attempt"));
		self.command().args(["recover", TASK]).output().unwrap()
	}

	fn assert_blocked(&self) {
		let before = fs::read(self.effects()).unwrap();
		let result = self.recover();
		assert!(!result.status.success());
		assert!(
			String::from_utf8_lossy(&result.stdout).contains("cannot confirm Git finished"),
			"{result:?}"
		);
		assert_eq!(fs::read(self.effects()).unwrap(), before);
	}
}

fn executable(path: &Path, bytes: &[u8]) {
	fs::write(path, bytes).unwrap();
	fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

/// Consume evidence through the state adapter under the normal admission lock.
/// Returning without a match leaves ordinary recovery's pending-writer guard in charge.
fn apply_success_receipt(state: &Path, attempt: &Path) {
	let store = Store::open_existing(state, None).unwrap();
	let guard = store.admit(TASK).unwrap();
	let record = store.load(&guard, TASK).unwrap();
	let Some(index) = record.pending else { return };
	let mut task = record.task;
	if task.operation.kind != Some(OperationKind::Create)
		|| task.participants[index].phase != Phase::Creating
	{
		return;
	}

	let result = match fs::metadata(attempt.join("success")) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == io::ErrorKind::NotFound => return,
		Err(error) => panic!("read completion receipt: {error}"),
	};
	let current = fs::metadata(guard.evidence_path.with_extension("effects")).unwrap();
	if (result.dev(), result.ino()) != (current.dev(), current.ino()) {
		return;
	}

	let member = &mut task.participants[index];
	member.worktree_identity = Some(fs::read_to_string(attempt.join("identity")).unwrap());
	member.phase = Phase::WorktreeCreated;
	store
		.record_effect(&guard, &task, index, EffectState::Settled)
		.unwrap();
}

#[test]
#[ignore = "Linux writer-exit prototype requires Python 3.9+ and child subreaper support"]
fn adopts_the_registered_tree_only_after_git_and_its_descendants_exit() {
	for case in ["git", "child"] {
		let fixture = Fixture::new();
		fixture.crash_at(case, "hook-entered");
		fixture.assert_blocked();
		fixture.release("release-git");
		fixture.wait("git-exited");
		if case == "child" {
			fixture.assert_blocked();
			fixture.release("release-child");
		}
		fixture.wait("attempt/success");

		let tree = fixture.root.join("tasks").join(TASK).join("api");
		fs::write(tree.join("notes"), "authored after crash").unwrap();
		assert_ok(&fixture.recover());
		assert_ok(&fixture.recover());
		assert_eq!(
			fs::read(tree.join("notes")).unwrap(),
			b"authored after crash"
		);
		assert_eq!(git(&tree, &["branch", "--show-current"]), TASK);
		assert_eq!(
			fs::read(fixture.dir.path().join("adds")).unwrap(),
			b"worktree add\n"
		);
		let published = support::record(&fixture.state());
		assert_eq!(published["participants"][0]["phase"], "completed");
		assert!(published["participants"][0]["worktree_identity"].is_string());
	}
}

#[test]
#[ignore = "Linux writer-exit prototype requires Python 3.9+ and child subreaper support"]
fn failed_add_does_not_authorize_adoption() {
	let fixture = Fixture::new();
	fixture.crash_at("failure", "hook-entered");
	fixture.release("release-git");
	fixture.wait("add-failed");
	fixture.assert_blocked();
}

#[test]
#[ignore = "Linux writer-exit prototype requires Python 3.9+ and child subreaper support"]
fn success_for_an_earlier_effect_cannot_settle_an_identical_new_record() {
	let fixture = Fixture::new();
	fixture.crash_at("git", "hook-entered");
	fixture.release("release-git");
	fixture.wait("attempt/success");
	let effects = fixture.effects();
	let replacement = effects.with_extension("new");
	fs::write(&replacement, fs::read(&effects).unwrap()).unwrap();
	fs::rename(replacement, effects).unwrap();
	fixture.assert_blocked();
}

#[test]
#[ignore = "Linux writer-exit prototype requires Python 3.9+ and child subreaper support"]
fn losing_the_supervisor_before_launch_leaves_the_effect_pending() {
	let fixture = Fixture::new();
	fixture.crash_at("prelaunch", "before-launch");
	let pid = fs::read_to_string(fixture.dir.path().join("supervisor-pid")).unwrap();
	assert_ok(&Command::new("kill").args(["-KILL", &pid]).output().unwrap());

	fixture.assert_blocked();
	assert!(!fixture.root.join("tasks").join(TASK).join("api").exists());
}
