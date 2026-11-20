use super::{client::Client, coordination::task_documents, support};
use serde_json::{Value, json};
use std::{
	fs,
	path::{Path, PathBuf},
};
use tempfile::TempDir;

mod file_changes;
mod interruptions;
mod scope;
mod submodules;

const TASK: &str = "feat/discard";

struct Workspace {
	fixture: TempDir,
	root: PathBuf,
}

impl Workspace {
	fn new(names: &[&str]) -> Self {
		let fixture = tempfile::tempdir().unwrap();
		let root = fixture.path().join("workspace");
		support::init_workspace(&root);
		let repositories: Vec<_> = names
			.iter()
			.map(|name| json!({"path":support::repo(fixture.path(), name)}))
			.collect();

		let mut client = Client::launch(&root);
		let create = client.call(
			"create",
			json!({"task": TASK, "repositories": repositories, "from": "main"}),
		);
		assert_ne!(create["isError"], true, "{create}");
		client.close();
		Self { fixture, root }
	}

	fn tree(&self, name: &str) -> PathBuf {
		self.root.join("tasks").join(TASK).join(name)
	}

	fn source(&self, name: &str) -> PathBuf {
		self.fixture.path().join(name)
	}

	fn state(&self) -> PathBuf {
		self.root.join(".workyard")
	}
}

fn decision(reply: &Value) -> &str {
	assert_eq!(reply["isError"], true, "{reply}");
	let outcome = &reply["structuredContent"];
	assert_eq!(outcome["problem"]["code"], "decision_required", "{reply}");
	let token = outcome["decision"]["token"]
		.as_str()
		.expect("disclosure token");
	assert!(!token.is_empty());
	token
}

fn disclosed_paths(reply: &Value, participant: &str) -> Vec<PathBuf> {
	let disclosures = reply["structuredContent"]["disclosures"]
		.as_array()
		.unwrap();
	let disclosure = disclosures
		.iter()
		.find(|d| d["participant"] == participant)
		.unwrap();
	let paths: Vec<trine::NativePath> =
		serde_json::from_value(disclosure["dirty_paths"].clone()).unwrap();
	paths.into_iter().map(|p| p.as_path().to_owned()).collect()
}

#[test]
fn removal_approval_survives_reconnect_only_for_the_reviewed_files() {
	let workspace = Workspace::new(&["api", "web"]);
	let web = workspace.tree("web");
	let excludes = workspace.source("web").join(".git/info/exclude");
	fs::write(excludes, "cache/\n").unwrap();
	fs::create_dir_all(web.join("cache/editor")).unwrap();
	let draft = web.join("cache/editor/draft");
	fs::write(&draft, "first").unwrap();

	let before = task_documents(&workspace.state());
	let mut client = Client::launch(&workspace.root);
	let progress_token = "inspect-task";
	let id = client.start_observed("remove", json!({"task": TASK}), progress_token);
	client.progress(progress_token, 1, "Inspected worktree for api");
	client.progress(progress_token, 2, "Inspected worktree for web");
	let initial = client.reply(id)["result"].take();
	let token = decision(&initial);
	assert_eq!(
		disclosed_paths(&initial, "web"),
		[Path::new("cache/editor/draft")]
	);
	assert_eq!(task_documents(&workspace.state()), before);
	client.close();

	fs::write(&draft, "later").unwrap();
	let mut client = Client::launch(&workspace.root);
	let stale = client.call("remove", json!({"task":TASK, "approve_discard":token}));
	let current = decision(&stale);
	assert_ne!(current, token);
	assert_eq!(fs::read_to_string(&draft).unwrap(), "later");
	assert_eq!(task_documents(&workspace.state()), before);
	client.close();

	let mut client = Client::launch(&workspace.root);
	let approval = json!({"task": TASK, "approve_discard": current});
	let progress_token = "apply-reviewed-removal";
	let id = client.start_observed("remove", approval, progress_token);
	client.progress(progress_token, 1, "Inspected worktree for api");
	client.progress(progress_token, 2, "Inspected worktree for web");
	client.progress(progress_token, 3, "Unlinked worktree for api");
	client.progress(progress_token, 4, "Unlinked worktree for web");

	let retired = client.reply(id)["result"].take();
	assert_ne!(retired["isError"], true, "{retired}");
	assert_eq!(retired["structuredContent"]["lifecycle"], "retired");
	for name in ["api", "web"] {
		assert!(!workspace.tree(name).exists());
		support::git(
			&workspace.source(name),
			&["show-ref", "--verify", "refs/heads/feat/discard"],
		);
	}
	client.close();
}

#[test]
fn changing_only_the_staged_blob_requires_a_new_disclosure() {
	let workspace = Workspace::new(&["api", "web"]);
	let api = workspace.tree("api");
	let file = api.join("file");
	fs::write(&file, "staged version one").unwrap();
	support::git(&api, &["add", "file"]);
	fs::write(&file, "working version").unwrap();
	let mut client = Client::launch(&workspace.root);
	let initial = client.call("amend", json!({"task":TASK, "remove":["api"]}));
	let token = decision(&initial);

	fs::write(&file, "staged version two").unwrap();
	support::git(&api, &["add", "file"]);
	fs::write(&file, "working version").unwrap();
	let stale = client.call(
		"amend",
		json!({"task":TASK, "remove":["api"], "approve_discard":token}),
	);
	assert_ne!(decision(&stale), token);
	assert_eq!(support::git(&api, &["show", ":file"]), "staged version two");
	assert_eq!(fs::read_to_string(file).unwrap(), "working version");
	client.close();
}

#[cfg(unix)]
#[test]
fn dirty_paths_preserve_native_filename_bytes() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

	let workspace = Workspace::new(&["api"]);
	let name = Path::new(OsStr::from_bytes(b"draft-\xff"));
	fs::write(workspace.tree("api").join(name), "authored").unwrap();
	let mut client = Client::launch(&workspace.root);
	let dirty = client.call("remove", json!({"task":TASK}));
	decision(&dirty);
	assert_eq!(disclosed_paths(&dirty, "api"), [name]);
	client.close();
}
