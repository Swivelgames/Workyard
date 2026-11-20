//! Exercise the gap between disclosure, a Git unlink and the next durable step.
use super::{create, state_bytes};
use crate::support::{assert_ok, git, record, repo, text, workyard};
use std::{
	fs,
	path::{Path, PathBuf},
	sync::atomic::{AtomicBool, Ordering},
};
use workyard_core::*;

fn amend_selected(
	root: &Path,
	names: &[&str],
	repositories: Vec<RepositoryInput>,
	observer: &impl RemovalObserver,
) -> AmendResult {
	let request = AmendRequest {
		task: "feat/task".into(),
		repositories,
		options: CreationOptions {
			checkout: false,
			no_fetch: true,
		},
		remove: names.iter().map(|name| (*name).to_owned()).collect(),
		removal_options: RemovalOptions {
			force: true,
			..Default::default()
		},
		remote: None,
		from: None,
	};
	let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
	smol::block_on(amend(
		request,
		&workyard_git::Runner::default(),
		&Control::default(),
		observer,
		&store,
	))
	.unwrap()
}

struct EditAfterDisclosure {
	path: PathBuf,
	edited: AtomicBool,
}

impl Observer for EditAfterDisclosure {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Ok(())
	}
}

impl RemovalObserver for EditAfterDisclosure {
	async fn disclose(&self, _: &str, _: &RemovalObservation) -> Result<(), Problem> {
		if !self.edited.swap(true, Ordering::SeqCst) {
			fs::write(&self.path, "edited after disclosure").unwrap();
		}
		Ok(())
	}

	async fn confirm(&self, question: Decision) -> Result<Option<bool>, Problem> {
		assert_eq!(question.kind, DecisionKind::DiscardFiles);
		Ok(Some(false))
	}
}

#[test]
fn force_cannot_discard_content_changed_after_disclosure() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	let web = repo(fixture.path(), "web");
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--repo", text(&web), "--from", "main"],
	));
	let path = root.join("tasks/feat/task/web/file");
	fs::write(&path, "original disclosure").unwrap();
	let before = state_bytes(&root);
	let observer = EditAfterDisclosure {
		path: path.clone(),
		edited: AtomicBool::new(false),
	};

	let result = amend_selected(&root, &["web"], Vec::new(), &observer);
	assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
	assert!(!result.mutation_started);
	assert_eq!(state_bytes(&root), before);
	assert_eq!(fs::read_to_string(path).unwrap(), "edited after disclosure");
}

struct StopAfterUnlink;

impl RemovalObserver for StopAfterUnlink {}

impl Observer for StopAfterUnlink {
	async fn phase(&self, _: &str, _: &str, phase: &str) -> Result<(), Problem> {
		if phase == "removed" {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				"Stop after Git unlinks the first tree.",
			));
		}
		Ok(())
	}
}

#[test]
fn named_recovery_resumes_only_outstanding_removals_with_fresh_consent() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	for name in ["web", "infra"] {
		let source = repo(fixture.path(), name);
		assert_ok(&workyard(
			&root,
			&[
				"amend",
				"feat/task",
				"--repo",
				text(&source),
				"--from",
				"main",
			],
		));
	}
	let result = amend_selected(&root, &["web", "infra"], Vec::new(), &StopAfterUnlink);
	assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
	assert!(result.mutation_started);
	let tasks = root.join("tasks/feat/task");
	let (remaining, removed) = if tasks.join("web").exists() {
		("web", "infra")
	} else {
		("infra", "web")
	};
	assert!(!tasks.join(removed).exists());
	let authored = tasks.join(remaining).join("authored");
	fs::write(&authored, "new edit").unwrap();

	let unchanged = state_bytes(&root);
	assert_ok(&workyard(&root, &["recover"]));
	assert_eq!(state_bytes(&root), unchanged);
	let response = workyard(&root, &["recover", "feat/task", "--yes", "--json"]);
	assert!(!response.status.success());
	let json: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
	assert_eq!(
		json["result"]["problem"]["code"], "decision_required",
		"{json}"
	);
	assert_eq!(fs::read_to_string(&authored).unwrap(), "new edit");
	let result = workyard(&root, &["recover", "feat/task", "--force", "--json"]);
	assert_ok(&result);
	let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
	for participant in [removed, remaining] {
		let expected = serde_json::json!({"participant":participant,"kind":"removed"});
		assert!(
			json["result"]["changes"]
				.as_array()
				.unwrap()
				.contains(&expected),
			"{json}"
		);
		assert!(!tasks.join(participant).exists());
		assert!(
			!git(
				&fixture.path().join(participant),
				&["branch", "--list", "feat/task"]
			)
			.is_empty()
		);
	}
	let saved = record(&root.join(".workyard"));
	assert_eq!(saved["participants"].as_array().unwrap().len(), 1);
	assert_eq!(saved["participants"][0]["name"], "api");
}

#[test]
fn named_recovery_finishes_a_staged_replacement_and_preserves_its_files() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let old = repo(fixture.path(), "api");
	let incoming = repo(fixture.path(), "next-api");
	create(&root, &old);
	let repo = RepositoryInput {
		path: incoming.clone(),
		name: Some("api".into()),
		from: Some("main".into()),
		remote: None,
		expected_identity: None,
		registration_roots: Vec::new(),
	};
	let interrupted = amend_selected(&root, &["api"], vec![repo], &StopAfterUnlink);
	assert_eq!(interrupted.operation.outcome, OperationOutcome::Cancelled);
	let staged = &interrupted.task.participants[0].destination;
	let destination = root.join("tasks/feat/task/api");
	assert_ne!(staged, &destination);
	assert!(!destination.exists());
	fs::write(staged.join("draft"), "edit made before recovery").unwrap();
	let administration = git(staged, &["rev-parse", "--absolute-git-dir"]);

	assert_ok(&workyard(&root, &["recover", "feat/task", "--yes"]));
	assert_eq!(
		fs::read_to_string(destination.join("draft")).unwrap(),
		"edit made before recovery"
	);
	assert_eq!(
		git(&destination, &["rev-parse", "--absolute-git-dir"]),
		administration
	);
	assert!(!staged.exists());
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["name"],
		"api"
	);
}

#[test]
fn undo_after_retirement_supersedes_outstanding_amendment_targets() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	// infra will be the first whole-task unlink and receive a new Git identity on undo.
	create(&root, &repo(fixture.path(), "infra"));
	for name in ["web", "api"] {
		let project = repo(fixture.path(), name);
		assert_ok(&workyard(
			&root,
			&[
				"amend",
				"feat/task",
				"--repo",
				text(&project),
				"--from",
				"main",
			],
		));
	}
	let partial = amend_selected(&root, &["web", "infra"], Vec::new(), &StopAfterUnlink);
	assert_eq!(partial.operation.outcome, OperationOutcome::Cancelled);
	let task_dir = root.join("tasks/feat/task");
	assert!(!task_dir.join("web").exists());
	assert!(task_dir.join("infra").exists());

	let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
	let retirement = smol::block_on(remove(
		"feat/task",
		RemovalOptions {
			yes: true,
			..Default::default()
		},
		&workyard_git::Runner::default(),
		&Control::default(),
		&StopAfterUnlink,
		&store,
	))
	.unwrap();
	assert_eq!(retirement.operation.outcome, OperationOutcome::Cancelled);
	assert!(!task_dir.join("infra").exists());
	assert_ok(&workyard(
		&root,
		&["recover", "feat/task", "--undo", "--yes"],
	));
	assert!(task_dir.join("infra/.git").is_file());
	assert!(!task_dir.join("web").exists());

	let guard = store.admit("feat/task").unwrap();
	let saved = store.load(&guard, "feat/task").unwrap();
	assert_eq!(saved.task.participants.len(), 2);
	assert!(store.amend_request(&guard).unwrap().is_none());
}
