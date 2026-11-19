use super::{create, registered, support::*};
use std::{fs, path::Path};

fn move_backing(workspace: &Path, original: &Path, moved: &Path) {
	fs::rename(original, moved).unwrap();
	assert_ok(&workyard(workspace, &["projects", "track", text(moved)]));
}

#[test]
fn recovery_preserves_an_independent_checkout_at_a_stale_registered_path() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let workspace = root.join("workspace");
	let original = repo(root, "api");
	let replacement = repo(root, "other");
	let side = root.join("side");

	init_workspace(&workspace);
	create(&workspace, "active", &original);
	git(
		&original,
		&["worktree", "add", "-b", "outside", text(&side)],
	);
	fs::rename(&side, root.join("previous-side")).unwrap();
	git(
		&replacement,
		&["worktree", "add", "-b", "independent", text(&side)],
	);
	fs::write(side.join("file"), "uncommitted").unwrap();
	let pointer = fs::read(side.join(".git")).unwrap();

	move_backing(&workspace, &original, &root.join("moved"));
	let result = workyard(&workspace, &["recover", "--activate-here"]);

	assert!(!result.status.success());
	assert_eq!(fs::read(side.join(".git")).unwrap(), pointer);
	assert_eq!(git(&side, &["branch", "--show-current"]), "independent");
	assert_eq!(fs::read(side.join("file")).unwrap(), b"uncommitted");
}

#[test]
fn one_recovery_reconnects_verified_peers_only_after_their_writers_release() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let workspace = root.join("workspace");
	let original = repo(root, "api");
	init_workspace(&workspace);
	for task in ["one", "two"] {
		create(&workspace, task, &original);
	}
	let second = workspace.join("tasks/two/api");
	let old_link = fs::read(second.join(".git")).unwrap();

	// A task lease can cover a writer even when Git currently holds no index.lock.
	let store = workyard_state::Store::open_existing(&workspace.join(".workyard"), None).unwrap();
	let lease = store.admit("two").unwrap();
	let moved = root.join("moved");
	move_backing(&workspace, &original, &moved);
	let result = workyard(&workspace, &["recover"]);
	assert!(!result.status.success());
	assert_eq!(fs::read(second.join(".git")).unwrap(), old_link);
	drop(lease);

	assert_ok(&workyard(&workspace, &["recover"]));
	for task in ["one", "two"] {
		let tree = workspace.join("tasks").join(task).join("api");
		assert_eq!(registered(&moved, task), tree.canonicalize().unwrap());
		assert_eq!(git(&tree, &["branch", "--show-current"]), task);
	}
}

#[cfg(unix)]
#[test]
fn recovery_preserves_a_backup_reached_through_a_peer_directory_symlink() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let workspace = root.join("workspace");
	let original = repo(root, "api");
	let side = root.join("side");
	let backup = root.join("backup");
	init_workspace(&workspace);
	create(&workspace, "active", &original);
	git(
		&original,
		&["worktree", "add", "-b", "outside", text(&side)],
	);

	super::copy_directory(&side, &backup);
	fs::rename(&side, root.join("previous-side")).unwrap();
	std::os::unix::fs::symlink(&backup, &side).unwrap();
	let pointer = fs::read(backup.join(".git")).unwrap();
	let active = workspace.join("tasks/active/api/.git");
	let active_pointer = fs::read(&active).unwrap();

	move_backing(&workspace, &original, &root.join("moved"));
	let recovery = workyard(&workspace, &["recover"]);

	assert!(!recovery.status.success());
	assert_eq!(fs::read(backup.join(".git")).unwrap(), pointer);
	assert_eq!(fs::read(active).unwrap(), active_pointer);
}
