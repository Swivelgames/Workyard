use super::create;
use crate::support::{assert_ok, record, repo, text, workyard};

struct CancelAfterRemoval;
impl workyard_core::Observer for CancelAfterRemoval {
	async fn phase(
		&self,
		_task: &str,
		_participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		if phase == "removed" {
			return Err(workyard_core::Problem::new(
				workyard_core::ProblemCode::Cancelled,
				"Stop after first removal.",
			));
		}
		Ok(())
	}
}
impl workyard_core::RemovalObserver for CancelAfterRemoval {
	async fn disclose(
		&self,
		_participant: &str,
		_observation: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
	async fn confirm(
		&self,
		_decision: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		Ok(Some(true))
	}
}

#[test]
fn amended_membership_survives_partial_removal_undo_and_later_amend() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let worker = repo(fixture.path(), "worker");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--from", "main", "--repo", text(&web)],
	));
	{
		let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
		let result = smol::block_on(workyard_core::remove(
			"feat/task",
			workyard_core::RemovalOptions {
				yes: true,
				force: false,
				undo: false,
			},
			&workyard_git::Runner::default(),
			&workyard_core::Control::new(|| false),
			&CancelAfterRemoval,
			&store,
		))
		.unwrap();
		assert_eq!(
			result.operation.outcome,
			workyard_core::OperationOutcome::Cancelled
		);
	}
	assert_ok(&workyard(&root, &["recover", "feat/task", "--undo"]));
	let completed = std::process::Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.env("TRINE_COMPLETE", "bash")
		.env("_CLAP_COMPLETE_INDEX", "4")
		.args(["--", "workyard", "amend", "feat/task", "--remove", ""])
		.output()
		.unwrap();
	assert_ok(&completed);
	let names = String::from_utf8(completed.stdout).unwrap();
	assert_eq!(names.split_whitespace().collect::<Vec<_>>(), ["api", "web"]);
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--from",
			"main",
			"--repo",
			text(&worker),
		],
	));
	assert_eq!(
		record(&root.join(".workyard"))["participants"]
			.as_array()
			.unwrap()
			.len(),
		3
	);
	assert_ok(&workyard(&root, &["status"]));
}
