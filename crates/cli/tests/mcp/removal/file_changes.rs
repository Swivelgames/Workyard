//! Disclose both index entries and working files when their names or types diverge.
use super::*;

#[test]
fn staged_rename_discloses_both_names() {
	let workspace = Workspace::new(&["api"]);
	support::git(&workspace.tree("api"), &["mv", "file", "renamed"]);
	let mut client = Client::launch(&workspace.root);
	let reply = client.call("remove", json!({"task": TASK}));
	decision(&reply);
	assert_eq!(
		disclosed_paths(&reply, "api"),
		[Path::new("file"), Path::new("renamed")]
	);
	client.close();
}

#[test]
fn a_staged_file_replaced_by_a_directory_discloses_the_index_and_children() {
	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let file = api.join("file");
	fs::write(&file, "index-only work").unwrap();
	support::git(&api, &["add", "file"]);
	fs::remove_file(&file).unwrap();
	fs::create_dir(&file).unwrap();
	fs::write(file.join("child"), "working file").unwrap();

	let mut client = Client::launch(&workspace.root);
	let reply = client.call("remove", json!({"task": TASK}));
	decision(&reply);
	assert_eq!(
		disclosed_paths(&reply, "api"),
		[Path::new("file"), Path::new("file/child")]
	);
	assert_eq!(support::git(&api, &["show", ":file"]), "index-only work");
	client.close();
}

fn tracked_module(api: &Path) -> PathBuf {
	let module = api.join("module");
	fs::create_dir(&module).unwrap();
	fs::write(module.join("old"), "tracked file").unwrap();
	support::git(api, &["add", "module"]);
	support::git(api, &["commit", "-m", "Track module"]);
	module
}

#[test]
fn a_directory_replaced_by_a_file_discloses_the_missing_descendants() {
	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let module = tracked_module(&api);
	fs::remove_file(module.join("old")).unwrap();
	fs::remove_dir(&module).unwrap();
	fs::write(&module, "new working file").unwrap();

	let mut client = Client::launch(&workspace.root);
	let reply = client.call("remove", json!({"task": TASK}));
	decision(&reply);
	assert_eq!(
		disclosed_paths(&reply, "api"),
		[Path::new("module"), Path::new("module/old")]
	);
	client.close();
}

#[cfg(unix)]
#[test]
fn executable_mode_change_invalidates_an_otherwise_identical_dirty_file() {
	use std::os::unix::fs::PermissionsExt;

	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	support::git(&api, &["config", "core.filemode", "true"]);
	let mut client = Client::launch(&workspace.root);
	let exclude = workspace.source("api").join(".git/info/exclude");
	fs::write(exclude, "ignored/\n").unwrap();
	fs::create_dir(api.join("ignored")).unwrap();

	for path in ["file", "untracked", "ignored/script"] {
		let file = api.join(path);
		fs::write(&file, "modified contents").unwrap();
		let initial = client.call("remove", json!({"task": TASK}));
		let token = decision(&initial);

		let mode = fs::metadata(&file).unwrap().permissions().mode();
		fs::set_permissions(&file, fs::Permissions::from_mode(mode ^ 0o100)).unwrap();
		let stale = client.call("remove", json!({"task": TASK, "approve_discard": token}));
		assert_ne!(decision(&stale), token, "{path}");
		assert_eq!(fs::read_to_string(file).unwrap(), "modified contents");
	}
	client.close();
}

#[cfg(unix)]
#[test]
fn a_symlink_in_place_of_a_tracked_directory_discloses_only_worktree_paths() {
	use std::os::unix::fs::symlink;

	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let module = tracked_module(&api);

	let other = workspace.fixture.path().join("other");
	fs::create_dir_all(other.join("old")).unwrap();
	fs::write(other.join("old/unrelated"), "belongs elsewhere").unwrap();
	fs::remove_file(module.join("old")).unwrap();
	fs::remove_dir(&module).unwrap();
	symlink(&other, &module).unwrap();

	let mut client = Client::launch(&workspace.root);
	let reply = client.call("remove", json!({"task": TASK}));
	decision(&reply);
	assert_eq!(
		disclosed_paths(&reply, "api"),
		[Path::new("module"), Path::new("module/old")]
	);
	assert_eq!(
		fs::read_to_string(other.join("old/unrelated")).unwrap(),
		"belongs elsewhere"
	);
	client.close();
}

#[test]
fn ignored_directory_expansion_preserves_nested_git_repositories() {
	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	fs::write(
		workspace.source("api").join(".git/info/exclude"),
		"cache/\n",
	)
	.unwrap();
	fs::create_dir_all(api.join("cache")).unwrap();
	let nested = support::repo(&api.join("cache"), "embedded");
	let before = task_documents(&workspace.state());
	let mut client = Client::launch(&workspace.root);
	let refused = client.call("remove", json!({"task": TASK}));

	assert_eq!(
		refused["structuredContent"]["problem"]["code"], "path_overlap",
		"{refused}"
	);
	assert!(refused["structuredContent"]["decision"]["token"].is_null());
	assert!(nested.join(".git").is_dir());
	assert_eq!(task_documents(&workspace.state()), before);
	client.close();
}

#[test]
fn an_empty_ignored_directory_survives_a_stale_approval() {
	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let excludes = workspace.source("api").join(".git/info/exclude");
	fs::write(excludes, "cache/\n").unwrap();
	fs::create_dir_all(api.join("cache/empty")).unwrap();
	fs::write(api.join("cache/draft"), "authored").unwrap();

	let mut client = Client::launch(&workspace.root);
	let initial = client.call("remove", json!({"task": TASK}));
	let token = decision(&initial);
	fs::rename(api.join("cache/empty"), api.join("cache/new-name")).unwrap();
	let stale = client.call("remove", json!({"task": TASK, "approve_discard": token}));

	assert_ne!(decision(&stale), token);
	assert!(api.join("cache/new-name").is_dir());
	assert_eq!(
		fs::read_to_string(api.join("cache/draft")).unwrap(),
		"authored"
	);
	client.close();
}

#[cfg(unix)]
#[test]
fn a_nested_bare_repository_stays_protected_with_linked_object_storage() {
	use std::os::unix::fs::symlink;

	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let nested = api.join("archive.git");
	support::git(&api, &["init", "--bare", "archive.git"]);
	let objects = workspace.fixture.path().join("objects");
	fs::rename(nested.join("objects"), &objects).unwrap();
	symlink(&objects, nested.join("objects")).unwrap();

	let mut client = Client::launch(&workspace.root);
	let refused = client.call("remove", json!({"task": TASK}));
	assert_eq!(
		refused["structuredContent"]["problem"]["code"], "path_overlap",
		"{refused}"
	);
	assert!(nested.join("HEAD").is_file());
	client.close();
}

#[cfg(unix)]
#[test]
fn a_displaced_gitlink_does_not_inspect_a_clone_outside_the_worktree() {
	use std::os::unix::fs::symlink;

	let workspace = Workspace::new(&["api"]);
	let api = workspace.tree("api");
	let module = api.join("module");
	fs::create_dir(&module).unwrap();
	let other = workspace.fixture.path().join("other");
	fs::create_dir(&other).unwrap();
	let unrelated = support::repo(&other, "dependency");
	let head = support::git(&unrelated, &["rev-parse", "HEAD"]);
	let gitlink = format!("160000,{head},module/dependency");
	support::git(&api, &["update-index", "--add", "--cacheinfo", &gitlink]);
	support::git(&api, &["commit", "-m", "Record dependency"]);
	fs::remove_dir(&module).unwrap();
	symlink(&other, &module).unwrap();

	let mut client = Client::launch(&workspace.root);
	let reply = client.call("remove", json!({"task": TASK}));
	decision(&reply);
	assert_eq!(
		disclosed_paths(&reply, "api"),
		[Path::new("module"), Path::new("module/dependency")]
	);
	assert!(unrelated.join(".git").is_dir());
	client.close();
}
