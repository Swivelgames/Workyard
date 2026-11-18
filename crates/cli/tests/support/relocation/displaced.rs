use super::*;
use std::sync::Mutex;
use workyard_core::{Observer, Problem, ProblemCode, RecoveryChoice};

/// Drive the same action policy as the CLI, supplying test decisions at the view boundary.
struct Decisions<F>(Mutex<F>);
impl<F> Observer for Decisions<F>
where
	F: FnMut(&RecoveryChoice) -> Result<bool, Problem> + Send,
{
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Ok(())
	}
	async fn choose_recovery(&self, choice: &RecoveryChoice) -> Result<bool, Problem> {
		(self.0.lock().unwrap())(choice)
	}
}

impl InterruptedCopy {
	fn displace(&self) -> PathBuf {
		let third = self.fixture.path().canonicalize().unwrap().join("outside");
		fs::rename(self.target.join("feat/one/api"), &third).unwrap();
		git(&self.api, &["worktree", "repair", "--", text(&third)]);
		third
	}

	fn run_recovery<F>(&self, undo: bool, choose: F) -> Result<(), Problem>
	where
		F: FnMut(&RecoveryChoice) -> Result<bool, Problem> + Send,
	{
		let mut state = Relocation::recover(&self.source.join(".workyard"))?;
		smol::block_on(workyard_core::relocate(
			&mut state,
			&workyard_git::Runner::default(),
			&Control::default(),
			undo,
			&Decisions(Mutex::new(choose)),
		))
	}
}

#[test]
fn equivalent_manually_registered_copy_resumes_in_both_directions() {
	for undo in [false, true] {
		let copy = InterruptedCopy::after_first_repair();
		let third = copy.displace();
		assert_ok(&copy.recover(undo));
		copy.assert_registered(if undo { &copy.source } else { &copy.target });
		assert!(third.join("file").is_file());
	}
}

#[test]
fn divergent_sources_supply_the_chosen_files_to_a_vacant_endpoint() {
	for use_registered in [false, true] {
		let copy = InterruptedCopy::after_first_repair();
		let third = copy.displace();
		let original = copy.source.join("feat/one/api");
		let dest = copy.target.join("feat/one/api");
		fs::write(original.join("notes"), "old").unwrap();
		fs::write(third.join("notes"), "scratch").unwrap();
		copy.run_recovery(false, |choice| {
			assert_eq!(choice.first, original);
			assert_eq!(choice.registered, third);
			assert_eq!(choice.destination, dest);
			assert!(!choice.occupied);
			Ok(use_registered)
		})
		.unwrap();
		copy.assert_registered(&copy.target);
		assert_eq!(
			fs::read_to_string(dest.join("notes")).unwrap(),
			if use_registered { "scratch" } else { "old" }
		);
		assert_eq!(fs::read_to_string(original.join("notes")).unwrap(), "old");
	}
}

#[test]
fn keep_or_overwrite_an_occupied_endpoint() {
	for use_registered in [false, true] {
		let copy = InterruptedCopy::after_first_repair();
		let third = copy.displace();
		let dest = copy.source.join("feat/one/api");
		fs::write(dest.join("only-at-destination"), "local").unwrap();
		fs::write(third.join("only-at-scratch"), "scratch").unwrap();
		copy.run_recovery(true, |choice| {
			assert_eq!(choice.first, dest);
			assert_eq!(choice.destination, dest);
			assert!(choice.occupied);
			Ok(use_registered)
		})
		.unwrap();
		copy.assert_registered(&copy.source);
		assert_eq!(dest.join("only-at-destination").exists(), !use_registered);
		assert_eq!(dest.join("only-at-scratch").exists(), use_registered);
	}
}

#[test]
fn changed_candidate_requires_a_new_decision_and_cancellation_keeps_all_files() {
	let copy = InterruptedCopy::after_first_repair();
	let third = copy.displace();
	let dest = copy.target.join("feat/one/api");
	fs::write(third.join("notes"), "first").unwrap();
	let mut calls = 0;
	let cancelled = copy
		.run_recovery(false, |_| {
			calls += 1;
			if calls == 1 {
				fs::write(third.join("notes"), "changed while deciding").unwrap();
				Ok(true)
			} else {
				Err(Problem::new(ProblemCode::Cancelled, "Cancelled"))
			}
		})
		.unwrap_err();
	assert_eq!(cancelled.code, ProblemCode::Cancelled);
	assert_eq!(calls, 2);
	assert!(!dest.exists());
	assert_eq!(
		fs::read_to_string(third.join("notes")).unwrap(),
		"changed while deciding"
	);
	assert!(git(&copy.api, &["worktree", "list"]).contains("outside"));
}

#[test]
fn noninteractive_recovery_reports_the_two_paths() {
	let copy = InterruptedCopy::after_first_repair();
	let third = copy.displace();
	fs::write(third.join("new"), "scratch").unwrap();
	let out = copy.recover(false);
	assert!(!out.status.success());
	let text = format!(
		"{}{}",
		String::from_utf8_lossy(&out.stdout),
		String::from_utf8_lossy(&out.stderr)
	);
	assert!(text.contains("outside"), "{text}");
	assert!(text.contains("api"), "{text}");
	assert!(!copy.target.join("feat/one/api").exists());
}

/// Stop between the two directory renames of an approved overwrite. Git still
/// registers scratch, the destination is vacant, and both versions are durable.
fn stop_before_publication(copy: &InterruptedCopy) -> (PathBuf, PathBuf) {
	use workyard_core::RelocationGit;
	let control = Control::default();
	let third = copy.displace();
	let dest = copy.source.join("feat/one/api");
	fs::write(dest.join("only-here"), "old").unwrap();
	fs::write(third.join("chosen"), "new").unwrap();
	let mut state = Relocation::recover(&copy.source.join(".workyard")).unwrap();
	let entry = state
		.participants()
		.into_iter()
		.find(|e| e.original.name == "api")
		.unwrap();
	let git = workyard_git::Runner::default();
	let registration = smol::block_on(git.displaced_copy(&entry, true, &control))
		.unwrap()
		.unwrap();
	let candidates = state
		.recovery_candidates(&entry, true, &registration, &control)
		.unwrap()
		.unwrap();
	state
		.select_recovery(&entry, true, &registration, &candidates, true)
		.unwrap();
	let parent = dest.parent().unwrap().to_owned();
	let stopped = Control::new(move || {
		fs::read_dir(&parent)
			.unwrap()
			.flatten()
			.any(|e| e.file_name().to_string_lossy().ends_with(".displaced"))
	});
	let result = state.install_recovery(&entry, true, &registration, &stopped);
	assert_eq!(result.unwrap_err().code, ProblemCode::Cancelled);
	assert!(!dest.exists());
	(dest, third)
}

#[test]
fn interrupted_overwrite_finishes_from_durable_selection() {
	let copy = InterruptedCopy::after_first_repair();
	let (dest, _) = stop_before_publication(&copy);
	assert_ok(&copy.recover(true));
	copy.assert_registered(&copy.source);
	assert!(dest.join("chosen").is_file());
	assert!(!dest.join("only-here").exists());
}

#[test]
fn changed_stage_pointer_cannot_replace_an_interrupted_destination() {
	let copy = InterruptedCopy::after_first_repair();
	let (dest, third) = stop_before_publication(&copy);
	let stage = fs::read_dir(dest.parent().unwrap())
		.unwrap()
		.flatten()
		.find(|e| e.file_name().to_string_lossy().ends_with(".stage"))
		.unwrap()
		.path();
	let foreign = fs::read(copy.source.join("feat/one/web/.git")).unwrap();
	fs::write(stage.join(".git"), &foreign).unwrap();
	assert!(!copy.recover(true).status.success());
	assert_eq!(fs::read(stage.join(".git")).unwrap(), foreign);
	assert!(third.join("chosen").exists());
	assert!(!dest.exists());
}

#[test]
fn replaced_endpoint_with_copied_pointer_is_ineligible() {
	let copy = InterruptedCopy::after_first_repair();
	let third = copy.displace();
	let old = copy.source.join("feat/one/api");
	let backup = copy.fixture.path().join("old-backup");
	fs::rename(&old, &backup).unwrap();
	fs::create_dir(&old).unwrap();
	fs::copy(backup.join(".git"), old.join(".git")).unwrap();
	fs::write(old.join("authored"), "unrelated").unwrap();
	let result = copy.recover(true);
	assert!(!result.status.success());
	assert!(String::from_utf8_lossy(&result.stdout).contains("replaced"));
	assert!(third.join("file").is_file());
	assert!(old.join("authored").is_file());
}

#[test]
fn one_remaining_candidate_reaches_vacant_destination() {
	let copy = InterruptedCopy::after_first_repair();
	let third = copy.displace();
	let old = copy.source.join("feat/one/api");
	fs::rename(old, copy.fixture.path().join("saved")).unwrap();
	fs::write(third.join("notes"), "remaining").unwrap();
	assert_ok(&copy.recover(false));
	copy.assert_registered(&copy.target);
	assert_eq!(
		fs::read_to_string(copy.target.join("feat/one/api/notes")).unwrap(),
		"remaining"
	);
}

#[test]
fn handoff_finished_before_git_repair_recovers_without_another_prompt() {
	use workyard_core::RelocationGit;
	let copy = InterruptedCopy::after_first_repair();
	let third = copy.displace();
	fs::write(third.join("selected"), "scratch").unwrap();
	let mut state = Relocation::recover(&copy.source.join(".workyard")).unwrap();
	let entry = state
		.participants()
		.into_iter()
		.find(|e| e.original.name == "api")
		.unwrap();
	let control = Control::default();
	let git = workyard_git::Runner::default();
	let registration = smol::block_on(git.displaced_copy(&entry, false, &control))
		.unwrap()
		.unwrap();
	let candidates = state
		.recovery_candidates(&entry, false, &registration, &control)
		.unwrap()
		.unwrap();
	state
		.select_recovery(&entry, false, &registration, &candidates, true)
		.unwrap();
	state
		.install_recovery(&entry, false, &registration, &control)
		.unwrap();
	drop(state);
	assert_ok(&copy.recover(false));
	copy.assert_registered(&copy.target);
	assert!(copy.target.join("feat/one/api/selected").is_file());
}

#[test]
fn switching_direction_settles_an_interrupted_file_publication() {
	let copy = InterruptedCopy::after_first_repair();
	let (old, third) = stop_before_publication(&copy);
	copy.run_recovery(false, |choice| {
		assert_eq!(choice.first, old);
		assert_eq!(choice.registered, third);
		Ok(true)
	})
	.unwrap();
	copy.assert_registered(&copy.target);
	assert!(old.join("only-here").is_file());
	assert!(copy.target.join("feat/one/api/chosen").is_file());
}

#[test]
fn different_link_groups_prompt_and_survive_installation() {
	let copy = InterruptedCopy::after_first_repair();
	let scratch = copy.displace();
	let source = copy.source.join("feat/one/api");
	for root in [&source, &scratch] {
		fs::write(root.join("a"), "identical").unwrap();
	}
	fs::copy(source.join("a"), source.join("b")).unwrap();
	fs::hard_link(scratch.join("a"), scratch.join("b")).unwrap();

	let mut asked = false;
	copy.run_recovery(false, |_| {
		asked = true;
		Ok(true)
	})
	.unwrap();
	assert!(asked);
	let active = copy.target.join("feat/one/api");
	fs::write(active.join("a"), "active edit").unwrap();
	assert_eq!(fs::read(active.join("b")).unwrap(), b"active edit");
	assert_eq!(fs::read(scratch.join("a")).unwrap(), b"identical");
}

#[cfg(unix)]
#[test]
fn overwrite_retires_a_verified_read_only_directory() {
	use std::os::unix::fs::PermissionsExt;

	let copy = InterruptedCopy::after_first_repair();
	let scratch = copy.displace();
	let read_only = copy.source.join("feat/one/api/read-only");
	fs::create_dir(&read_only).unwrap();
	fs::write(read_only.join("file"), "local only").unwrap();
	fs::set_permissions(&read_only, fs::Permissions::from_mode(0o555)).unwrap();
	fs::write(scratch.join("selected"), "registered").unwrap();

	copy.run_recovery(true, |_| Ok(true)).unwrap();
	copy.assert_registered(&copy.source);
	assert!(!read_only.exists());
}

fn save_scratch_selection(
	copy: &InterruptedCopy,
	undo: bool,
) -> (
	Relocation,
	workyard_core::RelocationParticipant,
	workyard_core::RegisteredCopy,
) {
	use workyard_core::RelocationGit;
	let mut state = Relocation::recover(&copy.source.join(".workyard")).unwrap();
	let entry = state
		.participants()
		.into_iter()
		.find(|e| e.original.name == "api")
		.unwrap();
	let control = Control::default();
	let registration =
		smol::block_on(workyard_git::Runner::default().displaced_copy(&entry, undo, &control))
			.unwrap()
			.unwrap();
	let candidates = state
		.recovery_candidates(&entry, undo, &registration, &control)
		.unwrap()
		.unwrap();
	state
		.select_recovery(&entry, undo, &registration, &candidates, true)
		.unwrap();
	(state, entry, registration)
}

#[test]
fn cancelling_second_participant_leaves_first_decision_unpublished() {
	let copy = InterruptedCopy::after_first_repair();
	let api = copy.displace();
	let web = copy.fixture.path().join("manual-web");
	fs::rename(copy.target.join("feat/one/web"), &web).unwrap();
	git(&copy.web, &["worktree", "repair", "--", text(&web)]);
	for path in [&api, &web] {
		fs::write(path.join("notes"), "manual").unwrap();
	}
	let mut count = 0;
	let error = copy
		.run_recovery(false, |_| {
			count += 1;
			if count == 2 {
				Err(Problem::new(ProblemCode::Cancelled, "cancelled"))
			} else {
				Ok(true)
			}
		})
		.unwrap_err();
	assert_eq!(error.code, ProblemCode::Cancelled);
	let journal: serde_json::Value =
		serde_json::from_slice(&fs::read(copy.source.join(".workyard/relocation.json")).unwrap())
			.unwrap();
	assert!(
		journal["participants"]
			.as_array()
			.unwrap()
			.iter()
			.all(|p| p.get("selection").is_none())
	);
}

#[test]
fn git_rebound_to_requested_endpoint_settles_unpublished_decision() {
	let copy = InterruptedCopy::after_first_repair();
	let scratch = copy.displace();
	fs::write(scratch.join("notes"), "manual").unwrap();
	drop(save_scratch_selection(&copy, true));
	let dest = copy.source.join("feat/one/api");
	git(&copy.api, &["worktree", "repair", "--", text(&dest)]);
	assert_ok(&copy.recover(true));
	copy.assert_registered(&copy.source);
	assert!(scratch.join("notes").is_file());
	assert!(!dest.join("notes").exists());
}

#[test]
fn opposite_direction_after_git_rebound_to_undo_endpoint() {
	use workyard_core::RelocationGit;
	for scratch_present in [false, true] {
		let copy = InterruptedCopy::after_first_repair();
		let scratch = copy.displace();
		fs::write(scratch.join("notes"), "manual").unwrap();
		let (mut state, entry, registered) = save_scratch_selection(&copy, true);
		let control = Control::default();
		state
			.install_recovery(&entry, true, &registered, &control)
			.unwrap();
		smol::block_on(workyard_git::Runner::default().bind_displaced_copy(
			&entry,
			true,
			&registered,
			&control,
		))
		.unwrap();
		drop(state);
		if !scratch_present {
			fs::rename(&scratch, copy.fixture.path().join("moved-aside")).unwrap();
		}
		assert_ok(&copy.recover(false));
		copy.assert_registered(&copy.target);
		assert!(copy.target.join("feat/one/api/notes").is_file());
	}
}

#[test]
fn changed_selected_source_reopens_choice_after_staging() {
	let copy = InterruptedCopy::after_first_repair();
	let (dest, scratch) = stop_before_publication(&copy);
	fs::write(scratch.join("chosen"), "newer").unwrap();
	let mut prompted = false;
	copy.run_recovery(true, |choice| {
		assert_eq!(choice.first, dest);
		prompted = true;
		Ok(true)
	})
	.unwrap();
	assert!(prompted);
	assert_eq!(fs::read_to_string(dest.join("chosen")).unwrap(), "newer");
}

#[test]
fn stage_created_before_identity_can_resume_or_offer_a_precise_remedy() {
	for empty in [true, false] {
		let copy = InterruptedCopy::after_first_repair();
		let third = copy.displace();
		fs::write(third.join("chosen"), "scratch").unwrap();
		let (state, _, _) = save_scratch_selection(&copy, true);
		let stage = copy
			.source
			.join("feat/one")
			.join(format!(".workyard-{}-0.stage", state.operation_id()));
		drop(state);
		fs::create_dir(&stage).unwrap();
		if !empty {
			fs::write(stage.join("authored"), "keep").unwrap();
			let result = copy.recover(true);
			assert!(!result.status.success());
			assert!(
				String::from_utf8_lossy(&result.stdout).contains(".stage"),
				"{result:?}"
			);
			assert_eq!(fs::read_to_string(stage.join("authored")).unwrap(), "keep");
			fs::rename(stage, copy.fixture.path().join("saved-stage")).unwrap();
		}
		assert_ok(&copy.recover(true));
		copy.assert_registered(&copy.source);
	}
}

#[test]
fn foreign_replacement_after_git_rebound_cannot_be_activated() {
	use workyard_core::RelocationGit;
	let copy = InterruptedCopy::after_first_repair();
	let scratch = copy.displace();
	fs::write(scratch.join("notes"), "manual").unwrap();
	let (mut state, entry, registered) = save_scratch_selection(&copy, true);
	let control = Control::default();
	state
		.install_recovery(&entry, true, &registered, &control)
		.unwrap();
	smol::block_on(workyard_git::Runner::default().bind_displaced_copy(
		&entry,
		true,
		&registered,
		&control,
	))
	.unwrap();
	drop(state);
	let dest = copy.source.join("feat/one/api");
	let saved = copy.fixture.path().join("real-worktree");
	fs::rename(&dest, &saved).unwrap();
	fs::create_dir(&dest).unwrap();
	fs::copy(saved.join(".git"), dest.join(".git")).unwrap();
	fs::write(dest.join("notes"), "other work").unwrap();
	let result = copy.recover(true);
	assert!(!result.status.success());
	assert_eq!(
		fs::read_to_string(dest.join("notes")).unwrap(),
		"other work"
	);
	assert_eq!(fs::read_to_string(saved.join("notes")).unwrap(), "manual");
}
