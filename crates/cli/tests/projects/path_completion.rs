use super::fixtures::suggestions;
use crate::support::{assert_ok, init_workspace, workyard};
use std::fs;

#[test]
fn untrack_completes_nested_roots_in_the_requesting_shell() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	init_workspace(workspace);
	assert_ok(&workyard(
		workspace,
		&["projects", "track", "teams/product"],
	));

	for shell in ["bash", "zsh", "fish", "powershell"] {
		let expected = if cfg!(windows) && shell == "powershell" {
			r"teams\product"
		} else {
			"teams/product"
		};
		let mut prefixes = vec!["tea", "teams/pro"];
		if cfg!(windows) {
			prefixes.push(r"teams\pro");
		}
		for prefix in prefixes {
			let args = ["projects", "untrack", prefix];
			assert_eq!(
				suggestions(workspace, shell, &args),
				[expected],
				"{shell}: {prefix}"
			);
		}
	}
}

#[test]
fn untrack_matches_the_typed_dot_in_the_selected_parent() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	init_workspace(workspace);
	assert_ok(&workyard(
		workspace,
		&["projects", "track", "projects/.local"],
	));

	for (prefix, expected) in [
		("proj/.", vec![]),
		("projects/..", vec![]),
		("projects/.", vec!["projects/.local"]),
	] {
		let args = ["projects", "untrack", prefix];
		assert_eq!(suggestions(workspace, "zsh", &args), expected, "{prefix}");
	}
}

#[cfg(windows)]
#[test]
fn verbatim_directory_completions_reopen_the_selected_directory() {
	use std::path::Path;

	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().canonicalize().unwrap();
	let directory = workspace.join("participant");
	fs::create_dir(&directory).unwrap();
	fs::write(directory.join("marker"), b"selected").unwrap();

	let prefix = workspace.join("part");
	for shell in ["bash", "zsh", "fish", "powershell"] {
		for flag in ["--repo", "--workspace"] {
			let args = ["create", flag, crate::support::text(&prefix)];
			let candidates = suggestions(&workspace, shell, &args);
			let [path] = candidates.as_slice() else {
				panic!("{shell} {flag}: {candidates:?}")
			};
			assert_eq!(Path::new(path), directory);
			assert_eq!(
				fs::read(Path::new(path).join("marker")).unwrap(),
				b"selected"
			);
		}
	}
}

#[cfg(unix)]
#[test]
fn literal_backslash_in_a_unix_directory_survives_shell_completion() {
	let fixture = tempfile::tempdir().unwrap();
	fs::create_dir(fixture.path().join(r"api\client")).unwrap();
	let args = ["create", "--repo", "api"];
	for shell in ["bash", "zsh", "fish", "powershell"] {
		assert_eq!(suggestions(fixture.path(), shell, &args), [r"api\client/"]);
	}
}
