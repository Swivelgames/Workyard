//! A retained workspace records the active endpoint without following it.
use super::{Relocation, conflict, record::Journal};
use crate::{Lock, documents::read_versioned, records::WorkspaceRecord};
use std::{
	fs,
	path::{Path, PathBuf},
};
use workyard_core::{Problem, ProblemCode};

pub struct SnapshotStatus {
	pub operation_id: String,
	pub tasks_root: PathBuf,
	pub active_tasks_root: PathBuf,
	pub active_state_dir: PathBuf,
	pub partial: bool,
}

impl SnapshotStatus {
	fn read(state: &Path, record: &WorkspaceRecord) -> Result<Option<Self>, Problem> {
		let partial = match record.authority.as_deref() {
			Some("partial_copy") => true,
			Some("snapshot") => false,
			_ => return Ok(None),
		};
		let invalid = || conflict("Inactive copy marker has inconsistent workspace anchors.");
		let active_state = if partial {
			&record.relocation_source
		} else {
			&record.relocation_destination
		};
		let active_state_dir = active_state.as_ref().ok_or_else(invalid)?.decode()?;
		let active_tasks_root = record
			.active_tasks_root
			.as_ref()
			.ok_or_else(invalid)?
			.decode()?;
		let tasks_root = record.tasks_root.decode()?;
		let operation_id = record
			.relocation
			.clone()
			.filter(|id| !id.is_empty())
			.ok_or_else(invalid)?;
		if record.state_dir.decode()? != state
			|| state == active_state_dir
			|| tasks_root == active_tasks_root
			|| !active_tasks_root.is_absolute()
			|| !active_state_dir.is_absolute()
			|| !tasks_root.is_absolute()
		{
			return Err(invalid());
		}
		Ok(Some(Self {
			operation_id,
			tasks_root,
			active_tasks_root,
			active_state_dir,
			partial,
		}))
	}
}

pub fn snapshot_status(state: &Path) -> Result<Option<SnapshotStatus>, Problem> {
	let manifest = state.join("workspace.json");
	if !crate::exists(&manifest)? {
		return Ok(None);
	}
	let record = WorkspaceRecord::read(&manifest)?;
	if !matches!(
		record.authority.as_deref(),
		Some("snapshot" | "partial_copy")
	) {
		return Ok(None);
	}
	let gate = Lock::read_existing(&state.join("workspace.lock"))?;
	let status = SnapshotStatus::read(state, &WorkspaceRecord::read(&manifest)?)?;
	gate.verify()?;
	Ok(status)
}

pub(crate) fn require_live(state: &Path, record: &WorkspaceRecord) -> Result<(), Problem> {
	if let Some(snapshot) = SnapshotStatus::read(state, record)? {
		let kind = if snapshot.partial {
			"partial copy"
		} else {
			"snapshot"
		};
		return Err(Problem::new(
			ProblemCode::WorkspaceBindingConflict,
			format!(
				"This workspace is an inactive {kind}. Active root: {:?}. Active state: {:?}.",
				snapshot.active_tasks_root, snapshot.active_state_dir
			),
		));
	}
	Ok(())
}

impl Relocation {
	pub(super) fn verify_copy_binding(
		&self,
		state: &Path,
		source: bool,
		binding: &WorkspaceRecord,
	) -> Result<(), Problem> {
		let root = if source {
			self.old_root()?
		} else {
			self.new_root()?
		};
		if binding.state_dir.decode()? != state || binding.tasks_root.decode()? != root {
			return Err(conflict(
				"Copy workspace binding disagrees with its expected location.",
			));
		}
		if binding.relocation.as_deref() == Some(&self.journal.id) {
			return Ok(());
		}
		if source {
			// Admission persists the journal immediately before publishing the pending
			// source binding. Accept the exact original manifest across that gap.
			let original = self
				.journal
				.files
				.iter()
				.find(|file| file.path == Path::new("workspace.json").into());
			let bytes = fs::read(state.join("workspace.json"))
				.map_err(|error| Problem::io("Verify original workspace binding", error))?;
			if original.is_some_and(|file| file.original == bytes) {
				return Ok(());
			}
		}
		Err(conflict(
			"Copy workspace binding belongs to another operation.",
		))
	}

	pub(super) fn retain_snapshot(&self, state: &Path, is_source: bool) -> Result<(), Problem> {
		let (root, authority) = if is_source {
			(self.old_root()?, "snapshot")
		} else {
			(self.new_root()?, "partial_copy")
		};
		let manifest = state.join("workspace.json");
		let journal_path = state.join("relocation.json");
		let has_journal = crate::exists(&journal_path)?;
		if has_journal {
			let paired: Journal = read_versioned(&journal_path)?;
			self.pair(&paired)?;
		}

		// The retained marker can already be durable if cleanup was interrupted.
		if crate::exists(&manifest)? {
			let binding = WorkspaceRecord::read(&manifest)?;
			self.verify_copy_binding(state, is_source, &binding)?;
			// The paired journal may be the only record of admission if the
			// process stopped just before marking the original source relocating.
			let original_source =
				is_source && has_journal && binding.relocation.as_deref() != Some(&self.journal.id);
			if !original_source
				&& !matches!(
					binding.authority.as_deref(),
					Some("relocating" | "snapshot" | "partial_copy")
				) {
				return Err(conflict(
					"Retained workspace binding disagrees with relocation.",
				));
			}
			if !has_journal && binding.authority.as_deref() != Some(authority) {
				return Err(conflict(
					"Retained workspace lacks its paired relocation intent.",
				));
			}
		} else if !has_journal {
			return Err(conflict(
				"Retained workspace lacks its paired relocation intent.",
			));
		}

		self.binding(state, &root, authority)?;
		if has_journal {
			fs::remove_file(journal_path)
				.map_err(|error| Problem::io("Remove completed snapshot intent", error))?;
		}
		Ok(())
	}
}
