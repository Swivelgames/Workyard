#[allow(dead_code)]
mod support;
use std::{
	collections::BTreeMap,
	ffi::{OsStr, OsString},
	fs,
	path::{Path, PathBuf},
	process::Command,
};
use support::*;

fn query(root: &Path, shell: &str, arguments: &[&OsStr], index: usize) -> Vec<String> {
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.env("TRINE_COMPLETE", shell)
		.env("_CLAP_COMPLETE_INDEX", index.to_string())
		.env_remove("TRINE_COMPLETE_LINE")
		.env_remove("TRINE_COMPLETE_METADATA")
		.env("GIT_CONFIG_NOSYSTEM", "1")
		.env(
			"GIT_CONFIG_GLOBAL",
			if cfg!(windows) { "NUL" } else { "/dev/null" },
		)
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

fn complete(root: &Path, shell: &str, arguments: &[&str], index: usize) -> Vec<String> {
	query(
		root,
		shell,
		&arguments.iter().map(OsStr::new).collect::<Vec<_>>(),
		index,
	)
}

fn metadata(root: &Path, name: &str, unique: &str) -> PathBuf {
	let path = repo(root, name);
	for remote in ["origin", unique] {
		git(
			&path,
			&[
				"remote",
				"add",
				remote,
				"file:///workyard-completion-offline-missing",
			],
		);
	}
	git(&path, &["branch", &format!("local/{unique}")]);
	git(&path, &["tag", "v1"]);
	git(
		&path,
		&[
			"update-ref",
			&format!("refs/remotes/origin/cached/{unique}"),
			"HEAD",
		],
	);
	git(
		&path,
		&[
			"update-ref",
			&format!("refs/remotes/{unique}/selected/{unique}"),
			"HEAD",
		],
	);
	git(
		&path,
		&[
			"symbolic-ref",
			"refs/remotes/origin/HEAD",
			&format!("refs/remotes/origin/cached/{unique}"),
		],
	);
	path
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	let mut result = BTreeMap::new();
	for entry in fs::read_dir(root).unwrap() {
		let path = entry.unwrap().path();
		if path.is_dir() {
			result.extend(files(&path));
		} else {
			result.insert(path.clone(), fs::read(path).unwrap());
		}
	}
	result
}

#[test]
fn per_repository_values_use_their_group_even_with_later_repositories() {
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api repo", "api");
	let web = metadata(root.path(), "web", "web");
	let before = files(root.path());

	for shell in ["bash", "zsh", "fish", "powershell"] {
		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"amend",
				"fix/one",
				"--repo",
				text(&api),
				"--repo-remote",
				"",
				"--repo",
				text(&web),
			],
			6,
		);
		assert_eq!(candidates, ["api", "origin"]);

		let inline = format!("--repo={}", text(&api));
		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"create",
				"fix/one",
				&inline,
				"--repo-from=",
				"--repo",
				text(&web),
			],
			4,
		);
		for expected in [
			"--repo-from=local/api",
			"--repo-from=origin/cached/api",
			"--repo-from=v1",
		] {
			assert!(
				candidates.contains(&expected.to_owned()),
				"{shell}: {candidates:?}"
			);
		}
		assert!(
			!candidates
				.iter()
				.any(|value| value.contains("web") || value.ends_with("HEAD"))
		);
	}

	assert_eq!(files(root.path()), before);
}

#[test]
fn source_names_qualify_branch_tag_and_remote_prefix_collisions() {
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api", "api");
	git(&api, &["branch", "origin/cached/api"]);
	git(&api, &["tag", "origin/cached/api"]);
	git(&api, &["tag", "local/api"]);
	git(&api, &["tag", "origin/tag-only"]);
	let before = files(root.path());

	for shell in ["bash", "zsh", "fish", "powershell"] {
		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"create",
				"task",
				"--repo",
				text(&api),
				"--repo-from",
				"",
			],
			6,
		);

		assert_eq!(
			candidates,
			[
				"api/selected/api",
				"local/api",
				"main",
				"origin/cached/api",
				"refs/heads/origin/cached/api",
				"refs/tags/local/api",
				"refs/tags/origin/cached/api",
				"refs/tags/origin/tag-only",
				"v1",
			],
			"{shell}"
		);
	}

	assert_eq!(files(root.path()), before);
}

#[test]
fn common_values_intersect_contributors_and_exclude_only_matching_overrides() {
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api", "api");
	let web = metadata(root.path(), "web", "web");
	let before = files(root.path());

	for shell in ["bash", "zsh", "fish", "powershell"] {
		for (flag, expected) in [("--remote", vec!["origin"]), ("--from", vec!["main", "v1"])] {
			let candidates = complete(
				root.path(),
				shell,
				&[
					"workyard",
					"create",
					"fix/one",
					flag,
					"",
					"--repo",
					text(&api),
					"--repo",
					text(&web),
				],
				4,
			);
			assert_eq!(candidates, expected, "{shell} {flag}");
		}

		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"amend",
				"fix/one",
				"--remote",
				"",
				"--repo",
				text(&api),
				"--repo",
				"missing",
				"--repo-remote=custom",
			],
			4,
		);
		assert_eq!(candidates, ["api", "origin"]);
		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"create",
				"fix/one",
				"--from",
				"local/",
				"--repo",
				text(&api),
				"--repo",
				"missing",
				"--repo-from=custom",
			],
			4,
		);
		assert_eq!(candidates, ["local/api"]);

		for tail in [
			vec!["--repo", "missing"],
			vec!["--repo", "missing", "--repo-from=custom"],
			vec!["--repo"],
		] {
			let mut args = vec![
				"workyard",
				"create",
				"fix/one",
				"--remote",
				"",
				"--repo",
				text(&api),
			];
			args.extend(tail);
			assert!(complete(root.path(), shell, &args, 4).is_empty());
		}
	}

	assert_eq!(files(root.path()), before);
}

#[test]
fn checkout_unions_branch_names_and_honors_selected_remotes_and_aliases() {
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api", "api");
	let web = metadata(root.path(), "web", "web");
	let before = files(root.path());

	for shell in ["bash", "zsh", "fish", "powershell"] {
		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"checkout",
				"",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
			],
			2,
		);
		assert_eq!(
			candidates
				.iter()
				.filter(|value| !value.starts_with('-'))
				.cloned()
				.collect::<Vec<_>>(),
			["cached/api", "cached/web", "local/api", "local/web", "main"]
		);
		assert!(
			!candidates
				.iter()
				.any(|value| value == "v1" || value.contains("HEAD"))
		);

		let candidates = complete(
			root.path(),
			shell,
			&[
				"workyard",
				"create",
				"selected/",
				"--checkout",
				"--remote=api",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
				"--repo-remote=web",
			],
			2,
		);
		assert_eq!(candidates, ["selected/api", "selected/web"]);
		for mode in [
			vec![],
			vec!["--checkout=false"],
			vec!["--checkout", "--checkout"],
		] {
			let mut args = vec!["workyard", "create", "cached/", "--repo", text(&api)];
			args.extend(mode);
			assert!(complete(root.path(), shell, &args, 2).is_empty());
		}
	}

	assert_eq!(files(root.path()), before);
}

#[test]
fn ambiguous_remote_selection_and_incomplete_or_literal_groups_yield_nothing() {
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api", "api");
	git(&api, &["remote", "rename", "origin", "other"]);

	assert!(
		complete(
			root.path(),
			"zsh",
			&["workyard", "checkout", "local/", "--repo", text(&api)],
			2
		)
		.is_empty()
	);
	assert_eq!(
		complete(
			root.path(),
			"zsh",
			&[
				"workyard",
				"checkout",
				"local/",
				"--repo",
				text(&api),
				"--repo-remote=api"
			],
			2
		),
		["local/api"]
	);
	for args in [
		vec![
			"workyard",
			"create",
			"task",
			"--remote",
			"",
			"--from=--repo",
			text(&api),
		],
		vec![
			"workyard",
			"create",
			"task",
			"--remote",
			"",
			"--",
			"--repo",
			text(&api),
		],
		vec![
			"workyard",
			"create",
			"task",
			"--repo",
			text(&api),
			"--repo-remote",
			"--repo",
		],
	] {
		let index = if args.contains(&"") { 4 } else { 6 };
		assert!(complete(root.path(), "zsh", &args, index).is_empty());
	}
}

#[test]
fn unsettled_pull_names_come_from_selected_task_evidence_and_placement() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	let web = repo(root.path(), "web");
	let state = root.path().join("external state");
	assert_ok(&workyard(
		root.path(),
		&[
			"create",
			"fix/one",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from=main",
			"--no-fetch",
			"--state-dir",
			text(&state),
		],
	));
	let filename = workyard_state::task_filename("fix/one");
	let path = state
		.join("evidence")
		.join(&filename)
		.with_extension("effects");
	let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	value["task"]["participants"][0]["phase"] = "pulling".into();
	fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

	let before = files(&state);

	for shell in ["bash", "zsh", "fish", "powershell"] {
		assert_eq!(
			complete(
				root.path(),
				shell,
				&[
					"workyard",
					"recover",
					"--accept-pull",
					"",
					"fix/one",
					"--state-dir",
					text(&state)
				],
				3
			),
			["api"]
		);
		assert!(
			complete(
				root.path(),
				shell,
				&[
					"workyard",
					"recover",
					"fix/missing",
					"--accept-pull",
					"",
					"--state-dir",
					text(&state)
				],
				4
			)
			.is_empty()
		);
	}

	assert_eq!(files(&state), before);
}

#[cfg(unix)]
#[test]
fn repository_completion_preserves_non_unicode_paths() {
	use std::os::unix::ffi::OsStringExt;
	let root = tempfile::tempdir().unwrap();
	let api = metadata(root.path(), "api", "api");
	let native = root.path().join(OsString::from_vec(b"repo-\xff".to_vec()));
	fs::rename(api, &native).unwrap();
	let arguments = [
		OsStr::new("workyard"),
		OsStr::new("create"),
		OsStr::new("task"),
		OsStr::new("--repo"),
		native.as_os_str(),
		OsStr::new("--repo-remote"),
		OsStr::new(""),
	];

	assert_eq!(query(root.path(), "zsh", &arguments, 6), ["api", "origin"]);
}
