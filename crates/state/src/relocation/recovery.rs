//! Resume paired authority under both gates; never reconstruct activated state.
use super::{Relocation, conflict, record::Journal};
use crate::{Lock, LockMode, canonical_future, documents::publish, records::WorkspaceRecord};
use std::{fs, path::Path};
use workyard_core::{Problem, RelocationState};

impl Relocation {
	pub fn recover(state: &Path) -> Result<Self, Problem> {
		let selected = canonical_future(state)?;
		if crate::exists(&selected.join("workspace.json"))? {
			super::snapshot::require_live(
				&selected,
				&WorkspaceRecord::read(&selected.join("workspace.json"))?,
			)?;
		}
		let selected_journal = selected.join("relocation.json");
		let journal: Journal = if selected_journal
			.try_exists()
			.map_err(|error| Problem::io("Inspect recovery intent", error))?
		{
			Journal::read(&selected_journal)?
		} else {
			let binding = WorkspaceRecord::read(&selected.join("workspace.json"))?;
			let destination = binding
				.relocation_destination
				.ok_or_else(|| conflict("No pending workspace relocation exists."))?
				.decode()?;
			let journal = Journal::read(&destination.join("relocation.json"))?;
			if binding.relocation.as_deref() != Some(&journal.id) {
				return Err(conflict("Inactive source routing identity disagrees."));
			}
			journal
		};

		let old_state = journal.old_state.decode()?;
		let new_state = journal.new_state.decode()?;
		if selected != old_state && selected != new_state {
			return Err(conflict(
				"Relocation anchors do not include selected state.",
			));
		}
		let source_gate = if old_state
			.try_exists()
			.map_err(|error| Problem::io("Inspect source state", error))?
		{
			Some(Lock::acquire_mode(
				&old_state.join("workspace.lock"),
				LockMode::Exclusive,
			)?)
		} else {
			None
		};
		let mut relocation = Self {
			journal,
			source_gate,
			destination_gate: None,
			started: true,
		};
		// The source is authority before activation; the selected staged journal can
		// be older than writer settlement or transfer progress.
		if old_state
			.join("relocation.json")
			.try_exists()
			.map_err(|error| Problem::io("Inspect authoritative intent", error))?
		{
			let current = Journal::read(&old_state.join("relocation.json"))?;

			relocation.pair(&current)?;
			relocation.journal = current;
		}
		if relocation.journal.undone
			&& old_state
				.join("workspace.json")
				.try_exists()
				.map_err(|error| Problem::io("Inspect undo authority", error))?
		{
			let restored = WorkspaceRecord::read(&old_state.join("workspace.json"))?;
			if restored.relocation.as_deref() == Some(&relocation.journal.id)
				&& restored.authority.as_deref() == Some("active")
			{
				if restored.state_dir.decode()? != old_state
					|| restored.tasks_root.decode()? != relocation.old_root()?
				{
					return Err(conflict(
						"Restored source binding disagrees with original anchors.",
					));
				}
				relocation.journal.activated = true;
			}
		}
		if new_state != old_state
			&& !(relocation.journal.undone
				&& relocation.journal.activated
				&& !new_state
					.try_exists()
					.map_err(|error| Problem::io("Inspect discarded destination", error))?)
		{
			if !new_state
				.try_exists()
				.map_err(|error| Problem::io("Inspect destination state", error))?
			{
				if relocation.journal.activated
					|| relocation.journal.activation_pending
					|| relocation.source_gate.is_none()
				{
					return Err(conflict(
						"Active destination is missing; stale source cannot reconstruct it.",
					));
				}
				let source = WorkspaceRecord::read(&old_state.join("workspace.json"))?;
				if source.authority.as_deref() == Some("inactive") {
					return Err(conflict(
						"Active destination is missing; inactive source cannot reconstruct it.",
					));
				}
				let create_state = if relocation.journal.copy {
					super::content::permissions::private_directories().create(&new_state)
				} else {
					fs::create_dir_all(&new_state)
				};
				create_state
					.map_err(|error| Problem::io("Recreate pending destination state", error))?;
			}
			relocation.destination_gate = Some(Lock::acquire_mode(
				&new_state.join("workspace.lock"),
				LockMode::Exclusive,
			)?);
			if relocation.journal.copy
				&& !relocation.journal.activated
				&& !crate::exists(&new_state.join("relocation.json"))?
			{
				// A user may remove the staged state directory before recovery.
				// Re-establish its paired intent only after confirming that the
				// gate is its sole entry. Occupied state belongs to the caller.
				for entry in fs::read_dir(&new_state)
					.map_err(|error| Problem::io("Inspect pending copy state", error))?
				{
					if entry
						.map_err(|error| Problem::io("Inspect copy state entry", error))?
						.file_name() != "workspace.lock"
					{
						return Err(conflict(
							"Destination state lacks paired copy intent and is occupied.",
						));
					}
				}
				publish(&new_state.join("relocation.json"), &relocation.journal)?;
			}
		}
		if new_state
			.join("relocation.json")
			.try_exists()
			.map_err(|error| Problem::io("Inspect destination intent", error))?
		{
			let paired = Journal::read(&new_state.join("relocation.json"))?;

			relocation.pair(&paired)?;
		}
		if crate::exists(&new_state.join("workspace.json"))? {
			let destination = WorkspaceRecord::read(&new_state.join("workspace.json"))?;
			if destination.relocation.as_deref() != Some(&relocation.journal.id) {
				return Err(conflict(
					"Destination authority belongs to another workspace.",
				));
			}
			if destination.authority.as_deref() == Some("active") {
				if relocation.journal.undone {
					if new_state != old_state {
						return Err(conflict(
							"Destination already activated; undo cannot reactivate the source.",
						));
					}
				} else {
					if destination.tasks_root.decode()? != relocation.new_root()?
						|| destination.state_dir.decode()? != new_state
					{
						return Err(conflict(
							"Active destination binding disagrees with relocation anchors.",
						));
					}
					relocation.journal.activated = true;
				}
			}
		}

		relocation.verify()?;
		if relocation.journal.copy && !relocation.journal.activated {
			let source = WorkspaceRecord::read(&old_state.join("workspace.json"))?;
			relocation.verify_copy_binding(&old_state, true, &source)?;
			if source.relocation.as_deref() != Some(&relocation.journal.id) {
				// Recover admission interrupted between the source journal and
				// publishing the matching pending source binding.
				relocation.binding(&old_state, &relocation.old_root()?, "relocating")?;
			}
		}
		Ok(relocation)
	}
}
