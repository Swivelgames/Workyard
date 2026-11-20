//! Submodule files and independent Git storage have different removal rules.
use super::*;

fn add_submodule(parent: &Path, dependency: &Path) {
	support::git(
		parent,
		&[
			"-c",
			"protocol.file.allow=always",
			"submodule",
			"add",
			support::text(dependency),
			"module",
		],
	);
	support::git(parent, &["commit", "-m", "Track dependency"]);
}

#[test]
fn removal_reviews_files_saved_inside_an_uninitialized_submodule() {
	let workspace = Workspace::new(&["api", "web"]);
	let api = workspace.tree("api");
	let source = workspace.source("api");
	let dependency = support::repo(workspace.fixture.path(), "dependency");
	add_submodule(&source, &dependency);
	support::git(&api, &["merge", "--ff-only", "main"]);

	// Both tools must return their disclosure even when the placeholder is empty.
	fs::write(api.join("draft"), "work in progress").unwrap();
	let note = api.join("module/note");
	let mut client = Client::launch(&workspace.root);
	let before = task_documents(&workspace.state());
	for (tool, input) in [
		("remove", json!({"task": TASK})),
		("amend", json!({"task": TASK, "remove": ["api"]})),
	] {
		let empty = client.call(tool, input.clone());
		decision(&empty);
		assert_eq!(disclosed_paths(&empty, "api"), [Path::new("draft")]);

		fs::write(&note, "first draft").unwrap();
		let populated = client.call(tool, input.clone());
		assert_eq!(
			disclosed_paths(&populated, "api"),
			[Path::new("draft"), Path::new("module/note")]
		);

		fs::write(&note, "later draft").unwrap();
		let updated = client.call(tool, input);
		assert_ne!(decision(&updated), decision(&populated));
		assert_eq!(task_documents(&workspace.state()), before);
		assert_eq!(fs::read_to_string(&note).unwrap(), "later draft");
		fs::remove_file(&note).unwrap();
	}

	fs::remove_file(api.join("draft")).unwrap();
	let clean = client.call("remove", json!({"task": TASK}));
	assert_ne!(clean["isError"], true, "{clean}");
	assert!(!api.exists());
	assert!(source.join(".git/modules/module").is_dir());
	client.close();
}

#[test]
fn deinitializing_a_submodule_preserves_its_local_git_history_on_removal() {
	let workspace = Workspace::new(&["api", "web"]);
	let api = workspace.tree("api");
	let dependency = support::repo(workspace.fixture.path(), "dependency");
	add_submodule(&api, &dependency);

	let module = api.join("module");
	support::git(
		&module,
		&[
			"-c",
			"user.name=Fixture",
			"-c",
			"user.email=test@example.invalid",
			"commit",
			"--allow-empty",
			"-m",
			"Local change",
		],
	);
	let head = support::git(&module, &["rev-parse", "HEAD"]);
	let git_dir = PathBuf::from(support::git(&module, &["rev-parse", "--absolute-git-dir"]));
	support::git(&api, &["commit", "-am", "Record local change"]);
	support::git(&api, &["submodule", "deinit", "--", "module"]);

	let before = task_documents(&workspace.state());
	let mut client = Client::launch(&workspace.root);
	for (tool, input) in [
		("remove", json!({"task": TASK})),
		("amend", json!({"task": TASK, "remove": ["api"]})),
	] {
		let refusal = client.call(tool, input);
		assert_eq!(
			refusal["structuredContent"]["problem"]["code"], "path_overlap",
			"{refusal}"
		);
		assert!(refusal["structuredContent"]["decision"]["token"].is_null());
		assert_eq!(task_documents(&workspace.state()), before);
	}

	// Even explicit permission to discard a working file cannot discard a repository.
	let draft = module.join("draft");
	fs::write(&draft, "authored").unwrap();
	for args in [
		vec!["remove", TASK, "--force"],
		vec!["amend", TASK, "--remove", "api", "--force"],
	] {
		let refused = support::workyard(&workspace.root, &args);
		assert!(!refused.status.success());
		assert_eq!(support::git(&git_dir, &["rev-parse", "HEAD"]), head);
		assert_eq!(support::git(&git_dir, &["cat-file", "-t", &head]), "commit");
		assert_eq!(fs::read_to_string(&draft).unwrap(), "authored");
		assert_eq!(task_documents(&workspace.state()), before);
	}
	client.close();
}
