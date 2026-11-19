use super::*;

#[test]
fn git_relative_links_can_move_or_copy_to_a_different_depth() {
	for copy in [false, true] {
		let fixture = tempfile::tempdir().unwrap();
		let old = fixture.path().join("original");
		let parent = fixture.path().join("several/layers");
		let new = parent.join("workspace");
		let api = repo(fixture.path(), "api");
		git(&api, &["config", "worktree.useRelativePaths", "true"]);
		init_workspace(&old);
		create(&old, "active", &api);
		let relative = Path::new("tasks/active/api");
		let pointer = fs::read_to_string(old.join(relative).join(".git")).unwrap();
		assert!(pointer.starts_with("gitdir: .."), "{pointer}");
		fs::create_dir_all(&parent).unwrap();
		if copy {
			copy_directory(&old, &new);
			let proposed = recover_rpc(&new, json!({}));
			assert_eq!(
				proposed["error"]["data"]["problem"]["code"], "decision_required",
				"{proposed}"
			);
			assert_eq!(
				registered(&api, "active"),
				old.join(relative).canonicalize().unwrap()
			);
			assert_ok(&workyard(&new, &["recover", "--activate-here"]));
			assert!(old.join(relative).join("file").is_file());
		} else {
			fs::rename(&old, &new).unwrap();
			assert_ok(&workyard(&new, &["recover"]));
		}
		assert_eq!(
			registered(&api, "active"),
			new.join(relative).canonicalize().unwrap()
		);
		assert_eq!(
			git(&new.join(relative), &["branch", "--show-current"]),
			"active"
		);
	}
}

#[test]
fn changed_remote_configuration_does_not_affect_local_placement() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("new");
	let api = repo(fixture.path(), "api");
	git(&api, &["remote", "add", "original", "."]);
	init_workspace(&old);
	create(&old, "active", &api);
	git(&api, &["remote", "remove", "original"]);
	for action in [
		&["status"][..],
		&["recover"][..],
		&["recover", "active"][..],
	] {
		assert_ok(&workyard(&old, action));
	}
	for name in ["first", "second"] {
		git(&api, &["remote", "add", name, "."]);
	}
	fs::rename(&old, &new).unwrap();
	assert_ok(&workyard(&new, &["recover"]));
	let status = workyard(&new, &["status"]);
	assert!(String::from_utf8_lossy(&status.stdout).contains("[healthy]"));
	assert_ok(&workyard(&new, &["remove", "active", "--force"]));
}

#[test]
fn old_volume_mismatch_preserves_the_unregistered_local_candidate() {
	use workyard_core::Control;
	use workyard_git::Runner;
	use workyard_state::Reader;
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("new");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	fs::rename(&old, &new).unwrap();
	let original = git(&api, &["worktree", "list", "--porcelain"]);

	let reader = Reader::open(&new.join(".workyard"), None).unwrap();
	let mut task = reader
		.tasks(Some("active"))
		.unwrap()
		.pop()
		.unwrap()
		.task
		.unwrap();
	let participant = &mut task.participants[0];
	let entry = participant
		.placement
		.as_mut()
		.unwrap()
		.worktree_entry
		.as_mut()
		.unwrap();
	entry.0 ^= 1;
	let problem = smol::block_on(Runner::default().inspect_placement(
		participant,
		None,
		&Control::new(|| false),
	))
	.unwrap_err();
	assert!(problem.message.contains("volume is unavailable"));
	assert_eq!(original, git(&api, &["worktree", "list", "--porcelain"]));
}

#[test]
fn incomplete_recovery_preserves_the_next_participants_placement_evidence() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("new");
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	init_workspace(&old);
	assert_ok(&workyard(
		&old,
		&[
			"create",
			"active",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
			"--no-fetch",
		],
	));
	let web_tree = Path::new("tasks/active/web");
	fs::write(old.join(web_tree).join("file"), "web edits").unwrap();
	let relocated_web = fixture.path().join("moved-web");
	fs::rename(&web, &relocated_web).unwrap();
	fs::rename(&old, &new).unwrap();

	let partial = recover_rpc(&new, json!({}));
	let rows = partial["error"]["data"]["participants"].as_array().unwrap();
	assert!(
		rows.iter()
			.any(|r| r["participant"] == "api" && r["state"] == "repaired"),
		"{partial}"
	);
	assert!(
		rows.iter()
			.any(|r| r["participant"] == "web" && r["state"] == "unresolved")
	);
	assert_eq!(
		registered(&api, "active"),
		new.join("tasks/active/api").canonicalize().unwrap()
	);

	assert_ok(&workyard(
		&new,
		&["projects", "track", text(&relocated_web)],
	));
	assert_ok(&workyard(&new, &["recover"]));
	assert_eq!(
		registered(&relocated_web, "active"),
		new.join(web_tree).canonicalize().unwrap()
	);
	assert_eq!(
		fs::read(new.join(web_tree).join("file")).unwrap(),
		b"web edits"
	);
}

#[test]
fn moving_a_relative_backing_clone_preserves_the_choice_between_live_workspace_copies() {
	let fixture = tempfile::tempdir().unwrap();
	let original = fixture.path().join("original");
	let copy = fixture.path().join("copy");
	let api = repo(fixture.path(), "api");
	git(&api, &["config", "worktree.useRelativePaths", "true"]);
	init_workspace(&original);
	create(&original, "active", &api);
	copy_directory(&original, &copy);

	let participant = Path::new("tasks/active/api");
	let original_tree = original.join(participant);
	let copied_tree = copy.join(participant);
	fs::write(original_tree.join("file"), "original edits").unwrap();
	fs::write(copied_tree.join("file"), "copy edits").unwrap();
	let parent = fixture.path().join("several/layers");
	fs::create_dir_all(&parent).unwrap();
	let moved_api = parent.join("api");
	fs::rename(&api, &moved_api).unwrap();
	assert_ok(&workyard(&copy, &["projects", "track", text(&moved_api)]));

	let before = git(&moved_api, &["worktree", "list", "--porcelain"]);
	let inspection = recover_rpc(&copy, json!({}));
	let error = &inspection["error"]["data"];
	assert_eq!(
		error["problem"]["code"], "decision_required",
		"{inspection}"
	);
	let proposed = &error["participants"][0];
	assert_eq!(
		proposed["registered"],
		json!(original_tree.canonicalize().unwrap())
	);
	assert_eq!(
		proposed["destination"],
		json!(copied_tree.canonicalize().unwrap())
	);
	assert_eq!(
		git(&moved_api, &["worktree", "list", "--porcelain"]),
		before
	);

	assert_ok(&workyard(&copy, &["recover", "--activate-here"]));
	assert_eq!(
		registered(&moved_api, "active"),
		copied_tree.canonicalize().unwrap()
	);
	assert_eq!(
		fs::read(original_tree.join("file")).unwrap(),
		b"original edits"
	);
	assert_eq!(fs::read(copied_tree.join("file")).unwrap(), b"copy edits");
}
