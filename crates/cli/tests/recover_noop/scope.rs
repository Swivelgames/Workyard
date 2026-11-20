//! Call the selected recovery entry directly to exercise its under-lock authorization.
use super::{create, snapshot, support};
use workyard_core::{Control, Observer, Problem, ProblemCode, RemovalObserver, RemovalOptions};

struct NoProgress;

impl Observer for NoProgress {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		panic!("An unrelated recovery route must stop before work begins");
	}
}

impl RemovalObserver for NoProgress {}

#[test]
fn stale_removal_recovery_cannot_retire_an_active_creation() {
	let fixture = tempfile::tempdir().unwrap();
	let source = support::repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	support::init_workspace(&root);
	create(&root, &source);

	let state = root.join(".workyard");
	let store = workyard_state::Store::open_existing(&state, None).unwrap();
	let before = snapshot(&state);
	let options = RemovalOptions {
		yes: true,
		..Default::default()
	};
	let result = smol::block_on(workyard_core::recover_removal(
		"feat/task",
		options,
		&workyard_git::Runner::default(),
		&Control::default(),
		&NoProgress,
		&store,
	));

	assert_eq!(result.err().unwrap().code, ProblemCode::InvalidInput);
	assert!(root.join("tasks/feat/task/api/.git").is_file());
	assert_eq!(snapshot(&state), before);
}

#[test]
fn stale_creation_recovery_cannot_replace_amendment_history() {
	let fixture = tempfile::tempdir().unwrap();
	let api = support::repo(fixture.path(), "api");
	let web = support::repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	support::init_workspace(&root);
	create(&root, &api);
	support::assert_ok(&support::workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--repo",
			support::text(&web),
			"--from",
			"main",
		],
	));

	let state = root.join(".workyard");
	let store = workyard_state::Store::open_existing(&state, None).unwrap();
	let before = snapshot(&state);
	let result = smol::block_on(workyard_core::recover(
		"feat/task",
		&workyard_git::Runner::default(),
		&Control::default(),
		&NoProgress,
		&store,
	));

	assert_eq!(result.err().unwrap().code, ProblemCode::InvalidInput);
	assert_eq!(snapshot(&state), before);
}
