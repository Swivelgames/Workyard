//! Race the public MCP call against the real repository relationship lock.
use super::*;
use crate::coordination::{StopAtPhase, hold_relationship_lock, wait_until};
use workyard_core::{
	Control, Observer, OperationOutcome, Problem, ProblemCode, RemovalObserver, RemovalOptions,
};

#[test]
fn disconnected_removal_resumes_with_only_the_remaining_worktree() {
	let workspace = Workspace::new(&["api", "web"]);
	let lock = hold_relationship_lock(&workspace.source("web"));
	let args = json!({"task": TASK});
	let mut client = Client::launch(&workspace.root);

	let token = "retire-task";
	client.start_observed("remove", args.clone(), token);
	client.progress(token, 1, "Inspected worktree for api");
	client.progress(token, 2, "Inspected worktree for web");
	client.progress(token, 3, "Unlinked worktree for api");
	assert!(!workspace.tree("api").exists());
	assert!(workspace.tree("web/.git").is_file());
	client.close();
	drop(lock);

	let mut client = Client::launch(&workspace.root);
	let status = client.call("status", args.clone());
	let operation = &status["structuredContent"]["tasks"][0]["recorded"]["operation"];
	assert_eq!(operation["kind"], "remove");
	assert_eq!(operation["outcome"], "cancelled");

	let token = "resume-retirement";
	let id = client.start_observed("remove", args, token);
	client.progress(token, 1, "Inspected worktree for web");
	client.progress(token, 2, "Unlinked worktree for web");
	let retired = client.reply(id)["result"].take();
	assert_ne!(retired["isError"], true, "{retired}");
	assert_eq!(retired["structuredContent"]["lifecycle"], "retired");
	assert!(!workspace.tree("web").exists());
	client.close();
}

#[test]
fn consent_is_rechecked_after_a_partial_unlink_at_the_git_lock() {
	for tool in ["remove", "amend"] {
		let workspace = Workspace::new(&["api", "web", "infra"]);
		fs::write(workspace.tree("api/old-draft"), "reviewed and removed").unwrap();
		let draft = workspace.tree("web/draft");
		fs::write(&draft, "first").unwrap();
		let mut client = Client::launch(&workspace.root);
		let mut args = json!({"task":TASK});
		if tool == "amend" {
			args["remove"] = json!(["api", "web"]);
		}
		let initial = client.call(tool, args.clone());
		args["approve_discard"] = json!(decision(&initial));

		let lock = hold_relationship_lock(&workspace.source("web"));
		let call = client.start_request("tools/call", json!({"name":tool, "arguments":args}));
		wait_until("first participant unlinked", || {
			!workspace.tree("api").exists()
		});
		fs::write(&draft, "changed before unlink").unwrap();
		drop(lock);

		let result = client.reply(call);
		let interrupted = &result["result"];
		let fresh = decision(interrupted);
		assert_ne!(fresh, decision(&initial));
		let remaining = interrupted["structuredContent"]["decision"]["participants"]
			.as_array()
			.unwrap();
		assert!(remaining.contains(&json!("web")));
		assert!(!remaining.contains(&json!("api")));
		let disclosures = interrupted["structuredContent"]["disclosures"]
			.as_array()
			.unwrap();
		let names: Vec<_> = disclosures
			.iter()
			.map(|item| item["participant"].as_str().unwrap())
			.collect();
		let expected = if tool == "remove" {
			vec!["infra", "web"]
		} else {
			vec!["web"]
		};
		assert_eq!(names, expected);
		assert_eq!(disclosed_paths(interrupted, "web"), [Path::new("draft")]);

		assert_eq!(fs::read_to_string(&draft).unwrap(), "changed before unlink");
		assert!(!workspace.tree("api").exists());
		client.close();

		let mut client = Client::launch(&workspace.root);
		args["approve_discard"] = json!(fresh);
		let resumed = client.call(tool, args);
		assert_ne!(resumed["isError"], true, "{resumed}");
		assert!(!workspace.tree("web").exists());
		assert_eq!(workspace.tree("infra/.git").is_file(), tool == "amend");
		client.close();
	}
}

struct StopAfterSecondRemoval;

impl Observer for StopAfterSecondRemoval {
	async fn phase(&self, _: &str, participant: &str, phase: &str) -> Result<(), Problem> {
		if participant == "web" && phase == "removed" {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				"Interrupted after removing web",
			));
		}
		Ok(())
	}
}

impl RemovalObserver for StopAfterSecondRemoval {}

#[test]
fn forward_removal_cannot_resume_an_interrupted_restore() {
	let workspace = Workspace::new(&["api", "web", "infra"]);
	let store = workyard_state::Store::open_existing(&workspace.state(), None).unwrap();
	let git = workyard_git::Runner::default();
	let control = Control::default();
	let retired = smol::block_on(workyard_core::remove(
		TASK,
		RemovalOptions::default(),
		&git,
		&control,
		&StopAfterSecondRemoval,
		&store,
	))
	.unwrap();
	assert_eq!(retired.operation.outcome, OperationOutcome::Cancelled);

	let restore = RemovalOptions {
		undo: true,
		..Default::default()
	};
	let restoring = smol::block_on(workyard_core::recover_removal(
		TASK,
		restore,
		&git,
		&control,
		&StopAtPhase("restored"),
		&store,
	))
	.unwrap();
	assert_eq!(restoring.operation.outcome, OperationOutcome::Cancelled);
	assert!(workspace.tree("api/.git").is_file());
	assert!(!workspace.tree("web").exists());
	let state = task_documents(&workspace.state());

	let mut client = Client::launch(&workspace.root);
	let refused = client.call("remove", json!({"task":TASK}));
	assert_eq!(refused["isError"], true, "{refused}");
	assert_eq!(task_documents(&workspace.state()), state);
	assert!(workspace.tree("api/.git").is_file());
	assert!(!workspace.tree("web").exists());
	client.close();

	support::assert_ok(&support::workyard(&workspace.root, &["recover", TASK]));
	assert!(workspace.tree("web/.git").is_file());
}

#[test]
fn a_waiting_git_unlink_rechecks_contents_and_retained_history() {
	use workyard_core::{RecoveryState, RemovalGit, UnlinkFailure};

	let workspace = Workspace::new(&["api"]);
	let store = workyard_state::Store::open_existing(&workspace.state(), None).unwrap();
	let guard = store.admit(TASK).unwrap();
	let record = store.load(&guard, TASK).unwrap();
	let member = &record.task.participants[0];
	let git = workyard_git::Runner::default();
	let control = Control::default();
	let draft = workspace.tree("api/draft");

	enum Change {
		Edit,
		Save,
		SetUpstream,
	}

	for change in [Change::Edit, Change::Save, Change::SetUpstream] {
		fs::write(&draft, "reviewed").unwrap();
		let disclosed = smol::block_on(git.inspect_removal(member, &control)).unwrap();
		let lock = hold_relationship_lock(&workspace.source("api"));
		let mut unlink = std::pin::pin!(git.remove_worktree(&disclosed, true, &control, || {
			panic!("an invalidated disclosure must stop before recording an unlink")
		}));
		assert!(smol::block_on(smol::future::poll_once(unlink.as_mut())).is_none());

		match change {
			Change::Edit => fs::write(&draft, "edited").unwrap(),
			Change::Save => fs::remove_file(&draft).unwrap(),
			Change::SetUpstream => {
				support::git(
					&workspace.tree("api"),
					&["branch", "--set-upstream-to=main"],
				);
			}
		}
		drop(lock);
		let failure = smol::block_on(unlink).unwrap_err();
		assert!(
			matches!(failure, UnlinkFailure::BeforeLaunch(problem) if problem.code == ProblemCode::DecisionRequired)
		);
		assert!(workspace.tree("api/.git").is_file());
	}
}
