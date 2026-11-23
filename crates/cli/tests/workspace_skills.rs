use serde_json::Value;
use std::{fs, path::Path, process::Command};

const INSTALLED_SKILL: &str = ".agents/skills/workyard/SKILL.md";

fn invoke(root: &Path, args: &[&str], exit: i32) -> Value {
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.args(args)
		.output()
		.unwrap();
	assert_eq!(output.status.code(), Some(exit), "{output:?}");
	serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
}

#[test]
fn init_installs_the_bundled_skill_and_preserves_local_edits() {
	let fixture = tempfile::tempdir().unwrap();
	let parent = fixture.path();
	let workspace = parent.join("workspace");
	let sibling = workspace.join(".agents/skills/swiss/SKILL.md");
	fs::create_dir_all(sibling.parent().unwrap()).unwrap();
	fs::write(&sibling, b"other product's skill").unwrap();

	invoke(
		parent,
		&["--json", "init", "workspace", "--tasks", "dev/trees"],
		0,
	);
	let installed = workspace.join(INSTALLED_SKILL);
	let original = fs::read_to_string(&installed).unwrap();
	assert!(original.lines().any(|line| line == "name: workyard"));
	assert!(!parent.join(".agents").exists());
	assert!(!workspace.join(".agent").exists());

	fs::write(&installed, b"customized skill").unwrap();
	invoke(&workspace, &["--json", "init"], 0);
	assert_eq!(fs::read(&installed).unwrap(), b"customized skill");
	assert_eq!(fs::read(&sibling).unwrap(), b"other product's skill");

	fs::remove_file(&installed).unwrap();
	invoke(&workspace, &["--json", "init"], 0);
	assert_eq!(fs::read_to_string(&installed).unwrap(), original);
}

#[test]
fn blocked_skill_install_preserves_the_workspace_and_can_be_retried() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	let blocker = workspace.join(".agents");
	fs::write(&blocker, b"existing file").unwrap();

	let failure = invoke(workspace, &["--json", "init"], 1);
	assert_eq!(failure["code"], "io_error");
	assert!(
		failure["remedy"]
			.as_str()
			.unwrap()
			.contains("rerun workyard init")
	);
	assert_eq!(fs::read(&blocker).unwrap(), b"existing file");
	invoke(workspace, &["--json", "list"], 0);

	fs::remove_file(&blocker).unwrap();
	invoke(workspace, &["--json", "init"], 0);
	assert!(workspace.join(INSTALLED_SKILL).is_file());
}

#[cfg(unix)]
#[test]
fn init_never_installs_through_a_linked_skill_directory() {
	use std::os::unix::fs::symlink;

	let workspace = tempfile::tempdir().unwrap();
	let outside = tempfile::tempdir().unwrap();
	symlink(outside.path(), workspace.path().join(".agents")).unwrap();

	invoke(workspace.path(), &["--json", "init"], 1);
	assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}

#[test]
fn tasks_path_cannot_occupy_the_skill_file() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();

	for tasks in [INSTALLED_SKILL, ".agents/skills/workyard/skill.md/trees"] {
		let failure = invoke(workspace, &["--json", "init", "--tasks", tasks], 1);
		assert_eq!(failure["code"], "path_overlap");
		assert!(!workspace.join(".workyard").exists());
	}

	invoke(workspace, &["--json", "init"], 0);
	assert!(workspace.join(INSTALLED_SKILL).is_file());
}
