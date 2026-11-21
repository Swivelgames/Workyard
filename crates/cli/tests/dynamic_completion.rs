#[allow(dead_code)]
mod support;
use std::{
	collections::BTreeMap,
	ffi::OsStr,
	fs,
	path::{MAIN_SEPARATOR, Path, PathBuf},
	process::Command,
};
use support::*;

fn query(root: &Path, shell: &str, index: usize, arguments: &[&OsStr]) -> Vec<String> {
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.env("TRINE_COMPLETE", shell)
		.env("_CLAP_COMPLETE_INDEX", index.to_string())
		.env_remove("TRINE_COMPLETE_LINE")
		.arg("--")
		.args(arguments)
		.output()
		.unwrap();
	assert_ok(&output);
	assert!(
		output.stderr.is_empty(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout)
		.unwrap()
		.split(if shell == "bash" { '\u{b}' } else { '\n' })
		.filter(|value| !value.is_empty())
		.map(str::to_owned)
		.collect()
}

fn tasks(root: &Path, command: &str, prefix: &str) -> Vec<String> {
	query(
		root,
		"zsh",
		2,
		&[
			OsStr::new("workyard"),
			OsStr::new(command),
			OsStr::new(prefix),
		],
	)
}

fn create(workspace: &Path, repo: &Path, name: &str) {
	assert_ok(&workyard(
		workspace,
		&[
			"create",
			name,
			"--repo",
			text(repo),
			"--from",
			"main",
			"--no-fetch",
		],
	));
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	let mut result = BTreeMap::new();
	for entry in fs::read_dir(root).unwrap() {
		let path = entry.unwrap().path();
		if path.is_dir() {
			result.insert(path.clone(), Vec::new());
			result.extend(files(&path));
		} else {
			result.insert(path.clone(), fs::read(path).unwrap());
		}
	}
	result
}

#[test]
fn recorded_tasks_are_live_read_only_and_distinct_from_files() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	init_workspace(root.path());
	let state = root.path().join(".workyard");
	for task in ["fix/two", "fix/one", "fix/retired"] {
		create(root.path(), &api, task);
	}
	assert_ok(&workyard(root.path(), &["remove", "fix/retired", "--yes"]));
	fs::create_dir_all(root.path().join("feat/decoy")).unwrap();
	let before = files(&state);

	for shell in ["bash", "zsh", "fish", "powershell"] {
		for action in ["amend", "remove", "recover", "status"] {
			assert_eq!(
				query(
					root.path(),
					shell,
					2,
					&["workyard".as_ref(), action.as_ref(), "fix/".as_ref()]
				),
				["fix/one", "fix/two"]
			);
			assert!(tasks(root.path(), action, "feat/").is_empty());
		}
		for action in ["create", "checkout"] {
			assert!(
				query(
					root.path(),
					shell,
					2,
					&["workyard".as_ref(), action.as_ref(), "feat/".as_ref()]
				)
				.is_empty()
			);
		}
		let paths = query(
			root.path(),
			shell,
			3,
			&[
				"workyard".as_ref(),
				"create".as_ref(),
				"--repo".as_ref(),
				"feat/".as_ref(),
			],
		);
		let separator = if shell == "powershell" {
			MAIN_SEPARATOR
		} else {
			'/'
		};
		let directory = format!("feat/decoy{separator}");
		assert!(paths.contains(&directory), "{shell}: {paths:?}");
	}
	assert_eq!(files(&state), before);
	let nested = root.path().join("feat/decoy");
	assert_eq!(tasks(&nested, "amend", "fix/"), ["fix/one", "fix/two"]);
}

#[test]
fn explicit_placement_after_cursor_and_missing_context_are_quiet() {
	let root = tempfile::tempdir().unwrap();
	let workspace = root.path().join("workspace");
	init_workspace(&workspace);
	let api = repo(root.path(), "api");
	let state = workspace.join(".workyard");
	create(&workspace, &api, "fix/one");
	let before = files(&state);
	let workspace_option = format!("--workspace={}", text(&workspace));
	for shell in ["bash", "zsh", "fish", "powershell"] {
		assert_eq!(
			query(
				root.path(),
				shell,
				2,
				&[
					"workyard".as_ref(),
					"amend".as_ref(),
					"fix/".as_ref(),
					workspace_option.as_ref()
				]
			),
			["fix/one"]
		);
		assert_eq!(
			query(
				root.path(),
				shell,
				2,
				&[
					"workyard".as_ref(),
					"amend".as_ref(),
					"fix/".as_ref(),
					"--workspace".as_ref(),
					workspace.as_os_str()
				]
			),
			["fix/one"]
		);
		assert!(
			query(
				root.path(),
				shell,
				2,
				&[
					"workyard".as_ref(),
					"amend".as_ref(),
					"fix/".as_ref(),
					"--workspace".as_ref()
				]
			)
			.is_empty()
		);
		assert!(
			query(
				root.path(),
				shell,
				2,
				&[
					"workyard".as_ref(),
					"amend".as_ref(),
					"fix/".as_ref(),
					"--workspace".as_ref(),
					root.path().as_os_str()
				]
			)
			.is_empty()
		);
	}
	assert_eq!(files(&state), before);
	assert!(tasks(root.path(), "amend", "fix/").is_empty());
	assert!(!root.path().join(".workyard").exists());
	let missing = root.path().join("missing");
	assert!(
		query(
			root.path(),
			"zsh",
			2,
			&[
				"workyard".as_ref(),
				"amend".as_ref(),
				"fix/".as_ref(),
				"--workspace".as_ref(),
				missing.as_os_str()
			]
		)
		.is_empty()
	);
	assert!(!missing.exists());
}

#[test]
fn busy_and_damaged_records_remain_suggestible_without_creating_gates() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	init_workspace(root.path());
	let state = root.path().join(".workyard");
	create(root.path(), &api, "fix/one");
	let filename = workyard_state::task_filename("fix/one");
	let record = state.join("tasks").join(&filename);
	let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
	value["problem"] =
		serde_json::json!({"code":"future_code", "message":"unknown diagnostic", "remedies":[]});
	fs::write(&record, serde_json::to_vec(&value).unwrap()).unwrap();
	let lock_path = state
		.join("locks")
		.join(Path::new(&filename).with_extension("lock"));
	let before = files(&state);
	let gate = workyard_state::Lock::acquire(&lock_path, false).unwrap();
	assert_eq!(tasks(root.path(), "recover", "fix/"), ["fix/one"]);
	drop(gate);
	assert_eq!(files(&state), before);
	fs::remove_file(&lock_path).unwrap();
	let before = files(&state);
	assert_eq!(tasks(root.path(), "recover", "fix/"), ["fix/one"]);
	assert_eq!(files(&state), before);
}

#[test]
fn bash_line_transport_reassembles_equals_and_colons_without_evaluation() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	let workspace = root.path().join("some workspace");
	init_workspace(&workspace);
	create(&workspace, &api, "fix/one");
	let line = format!("workyard amend f --workspace='{}'", text(&workspace));
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root.path())
		.env("TRINE_COMPLETE", "bash")
		.env("TRINE_COMPLETE_LINE", &line)
		.env("TRINE_COMPLETE_POINT", "workyard amend f".len().to_string())
		.arg("--")
		.output()
		.unwrap();
	assert_ok(&output);
	assert!(output.stderr.is_empty());
	assert_eq!(output.stdout, b"fix/one");
}

#[test]
fn evidence_only_records_and_inaccessible_workspace_do_not_change_state() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	init_workspace(root.path());
	let state = root.path().join(".workyard");
	create(root.path(), &api, "fix/one");
	let filename = workyard_state::task_filename("fix/one");
	fs::rename(
		state.join("tasks").join(&filename),
		state.join("evidence").join(filename),
	)
	.unwrap();
	fs::write(state.join("tasks/unreadable.json"), "broken record").unwrap();
	let before = files(&state);
	assert_eq!(tasks(root.path(), "recover", "fix/"), ["fix/one"]);
	assert_eq!(files(&state), before);

	let gate = workyard_state::Lock::acquire(&state.join("workspace.lock"), false).unwrap();
	assert!(tasks(root.path(), "recover", "fix/").is_empty());
	drop(gate);
	assert_eq!(files(&state), before);
	fs::remove_file(state.join("workspace.lock")).unwrap();
	let before = files(&state);
	assert!(tasks(root.path(), "recover", "fix/").is_empty());
	assert_eq!(files(&state), before);
}

#[cfg(unix)]
#[test]
fn native_workspace_paths_reach_discovery_without_lossy_conversion() {
	use std::os::unix::ffi::OsStringExt;
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	let workspace = root
		.path()
		.join(std::ffi::OsString::from_vec(b"state-\xff".to_vec()));
	init_workspace(&workspace);
	let state = workspace.join(".workyard");
	let created = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root.path())
		.args(["create", "fix/one", "--repo"])
		.arg(&api)
		.args(["--from", "main", "--no-fetch", "--workspace"])
		.arg(&workspace)
		.output()
		.unwrap();
	assert_ok(&created);
	let before = files(&state);
	assert_eq!(
		query(
			root.path(),
			"zsh",
			2,
			&[
				"workyard".as_ref(),
				"amend".as_ref(),
				"fix/".as_ref(),
				"--workspace".as_ref(),
				workspace.as_os_str()
			]
		),
		["fix/one"]
	);
	assert_eq!(files(&state), before);
}

#[test]
fn bash_path_candidates_replace_only_the_word_after_equals() {
	let root = tempfile::tempdir().unwrap();
	fs::create_dir(root.path().join("state folder")).unwrap();
	let line = "workyard info --workspace=sta";
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root.path())
		.env("TRINE_COMPLETE", "bash")
		.env("TRINE_COMPLETE_LINE", line)
		.env("TRINE_COMPLETE_POINT", line.len().to_string())
		.env("TRINE_COMPLETE_WORD", "sta")
		.arg("--")
		.output()
		.unwrap();
	assert_ok(&output);
	assert!(output.stderr.is_empty());
	assert_eq!(output.stdout, b"state folder/");
}

#[test]
fn runtime_catalog_preserves_host_commands_aliases_and_flags() {
	let root = tempfile::tempdir().unwrap();
	for shell in ["bash", "zsh", "fish", "powershell"] {
		let commands = query(root.path(), shell, 1, &["workyard".as_ref(), "".as_ref()]);
		for name in [
			"init",
			"amend",
			"create",
			"checkout",
			"recover",
			"remove",
			"list",
			"status",
			"info",
			"rpc",
			"completion",
		] {
			assert!(commands.contains(&name.to_owned()), "{shell}: {commands:?}");
		}
		let flags = query(
			root.path(),
			shell,
			2,
			&["workyard".as_ref(), "checkout".as_ref(), "--".as_ref()],
		);
		assert!(!flags.contains(&"--repo-name".to_owned()));
		for name in ["--repo", "--workspace", "--json"] {
			assert!(flags.contains(&name.to_owned()), "{shell}: {flags:?}");
		}
	}
	assert!(fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn group_children_follow_completed_repo_anchors_in_every_shell() {
	let root = tempfile::tempdir().unwrap();
	for shell in ["bash", "zsh", "fish", "powershell"] {
		for action in ["create", "checkout", "amend"] {
			for (before, after, eligible) in [
				(vec![], vec![], false),
				(vec!["--repo", "uninspected path"], vec![], true),
				(vec!["--repo=uninspected path"], vec![], true),
				(vec!["--repo"], vec![], false),
				(vec![], vec!["--repo", "uninspected path"], false),
				(vec!["--from=--repo"], vec![], false),
				(vec!["--from", "--repo", "uninspected path"], vec![], false),
				(vec!["--", "--repo", "uninspected path"], vec![], false),
			] {
				let mut arguments = vec!["workyard", action, "fix/one"];
				arguments.extend(before);
				let cursor = arguments.len();
				arguments.push("--repo-");
				arguments.extend(after);
				let native: Vec<_> = arguments.iter().map(OsStr::new).collect();

				let candidates = query(root.path(), shell, cursor, &native);
				for child in ["--repo-name", "--repo-remote", "--repo-from"] {
					assert_eq!(
						candidates.contains(&child.to_owned()),
						eligible,
						"{shell}: {arguments:?}: {candidates:?}"
					);
				}
			}
		}
	}

	assert!(fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn zsh_metadata_names_task_groups_without_changing_values() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	init_workspace(root.path());
	let state = root.path().join(".workyard");
	create(root.path(), &api, "fix/one");
	let before = files(&state);

	for action in ["amend", "remove", "recover", "status"] {
		let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
			.current_dir(root.path())
			.env("TRINE_COMPLETE", "zsh")
			.env("TRINE_COMPLETE_METADATA", "1")
			.env("_CLAP_COMPLETE_INDEX", "2")
			.args(["--", "workyard", action, ""])
			.output()
			.unwrap();
		assert_ok(&output);
		assert!(output.stderr.is_empty());

		let text = String::from_utf8(output.stdout).unwrap();
		let fields: Vec<_> = text.lines().collect();
		assert_eq!(&fields[..3], ["Tasks", "fix/one", "api"]);
		assert!(
			fields
				.chunks_exact(3)
				.any(|row| row[0] == "Options" && row[1] == "--help" && !row[2].is_empty())
		);
	}

	assert_eq!(files(&state), before);
}
