//! Durable adapter steps used by core relocation orchestration.
use super::{Relocation, admission::empty, conflict, content, metadata};
use crate::{Lock, LockMode, documents::publish};
use std::{
	fs,
	path::{Path, PathBuf},
};
use workyard_core::{Problem, RelocationParticipant, RelocationState};

impl RelocationState for Relocation {
	fn prior_registered_copy(
		&self,
		entry: &RelocationParticipant,
	) -> Result<Option<PathBuf>, Problem> {
		self.previous_registration(entry)
	}
	fn accept_registered_endpoint(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		control: &workyard_core::Control,
	) -> Result<(), Problem> {
		self.accept_endpoint(entry, undo, control)
	}

	fn settle_recovery_direction(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		registered: &workyard_core::RegisteredCopy,
		control: &workyard_core::Control,
	) -> Result<(), Problem> {
		self.settle_direction(entry, undo, registered, control)
	}

	fn recovery_candidates(
		&self,
		entry: &RelocationParticipant,
		undo: bool,
		registered: &workyard_core::RegisteredCopy,
		control: &workyard_core::Control,
	) -> Result<Option<workyard_core::RecoveryCandidates>, Problem> {
		self.candidates(entry, undo, registered, control)
	}
	fn select_recovery(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		registered: &workyard_core::RegisteredCopy,
		candidates: &workyard_core::RecoveryCandidates,
		use_registered: bool,
	) -> Result<(), Problem> {
		self.select(entry, undo, registered, candidates, use_registered)
	}
	fn install_recovery(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		registered: &workyard_core::RegisteredCopy,
		control: &workyard_core::Control,
	) -> Result<(), Problem> {
		self.install(entry, undo, registered, control)
	}
	fn is_copy(&self) -> bool {
		self.journal.copy
	}

	fn copy_participant_expected(&self, entry: &RelocationParticipant) -> bool {
		!self.journal.participants.iter().any(|record| {
			record.absent_at_admission
				&& record.task.task == entry.task
				&& record.destination == entry.destination.as_path().into()
		})
	}

	fn participants(&self) -> Vec<RelocationParticipant> {
		self.journal
			.participants
			.iter()
			.map(|entry| RelocationParticipant {
				task: entry.task.task.clone(),
				original: crate::records::task::decode(entry.task.clone())
					.expect("validated task")
					.participants[entry.index]
					.clone(),
				destination: entry.destination.decode().expect("validated native path"),
			})
			.collect()
	}
	fn started(&self) -> bool {
		self.started
	}
	fn relocated_path(&self, path: &Path) -> Result<PathBuf, Problem> {
		Ok(path.strip_prefix(self.old_root()?).map_or_else(
			|_| path.to_path_buf(),
			|relative| self.new_root().expect("validated root").join(relative),
		))
	}
	fn activated(&self) -> bool {
		self.journal.activated
	}
	fn pending_writer(&self) -> bool {
		self.journal.writer_pending
	}
	fn verify(&self) -> Result<(), Problem> {
		if let Some(gate) = &self.source_gate {
			gate.verify()?;
		}
		if let Some(gate) = &self.destination_gate {
			gate.verify()?;
		}
		if self.source_gate.is_none() && !self.journal.activated {
			return Err(conflict("Authoritative source state is missing."));
		}
		Ok(())
	}
	fn begin(&mut self, participants: Vec<RelocationParticipant>) -> Result<(), Problem> {
		self.verify()?;
		if self.started {
			return Ok(());
		}
		if participants.len() != self.journal.participants.len() {
			return Err(conflict(
				"Relocation participant count changed before admission.",
			));
		}
		if self.new_root()? != self.old_root()? {
			empty(&self.new_root()?)?;
		}

		let old_root = self.old_root()?;
		if self.new_root()? != old_root {
			for participant in &participants {
				if [
					&participant.original.source,
					&participant.original.common_dir,
				]
				.iter()
				.any(|path| path.starts_with(&old_root))
				{
					return Err(conflict(
						"Resolved backing repositories inside the moved root cannot be moved by workspace relocation.",
					));
				}
			}
		}

		let copy = self.is_copy();
		for (record, observed) in self.journal.participants.iter_mut().zip(participants) {
			let mut task = crate::records::task::decode(record.task.clone())?;
			if copy {
				record.absent_at_admission = !observed
					.original
					.destination
					.try_exists()
					.map_err(|error| Problem::io("Inspect original participant location", error))?;
			}
			task.participants[record.index] = observed.original;
			record.task = crate::TaskRecord::from(&task);
			record.destination = observed.destination.as_path().into();
		}
		metadata::transform(&mut self.journal)?;
		let old_state = self.old_state()?;
		let new_state = self.new_state()?;
		if new_state != old_state {
			empty(&new_state)?;
			let create_state = if copy {
				content::permissions::prepare_root(&self.new_root()?)?;
				content::permissions::private_directories().create(&new_state)
			} else {
				fs::create_dir_all(&new_state)
			};
			create_state.map_err(|error| Problem::io("Create destination state", error))?;
			self.destination_gate = Some(Lock::acquire_mode(
				&new_state.join("workspace.lock"),
				LockMode::Exclusive,
			)?);
			for entry in fs::read_dir(&new_state)
				.map_err(|error| Problem::io("Recheck protected destination", error))?
			{
				let entry =
					entry.map_err(|error| Problem::io("Recheck destination entry", error))?;
				if entry.file_name() != "workspace.lock" {
					return Err(conflict(
						"Destination became occupied before relocation intent.",
					));
				}
			}
		}
		// State scaffolding may share a parent with copied workspace entries.
		if copy {
			let root = self.new_root()?;
			for entry in &mut self.journal.content {
				let target = root.join(entry.path.decode()?);
				if new_state.starts_with(&target) {
					entry.destination_identity = Some(workyard_system::entry_identity(&target)?);
				}
			}
		}
		self.persist()?;
		self.started = true;
		self.binding(&old_state, &self.old_root()?, "relocating")?;
		if new_state != old_state {
			publish(&new_state.join("relocation.json"), &self.journal)?;
		}
		Ok(())
	}
	fn transfer(&mut self, undo: bool, control: &workyard_core::Control) -> Result<(), Problem> {
		self.verify()?;
		let old_state = self.old_state()?;
		let gate = self
			.source_gate
			.as_ref()
			.ok_or_else(|| conflict("Source gate is missing."))?;
		let publish_progress = |journal: &super::record::Journal| {
			gate.verify()?;
			publish(&old_state.join("relocation.json"), journal)
		};
		if self.journal.copy {
			return content::copy_workspace(&mut self.journal, undo, control, publish_progress);
		}
		content::transfer(&mut self.journal, undo, control, publish_progress)
	}
	fn copy_writer(&mut self, entry: &RelocationParticipant, undo: bool) -> Result<(), Problem> {
		let location = if undo {
			&entry.original.destination
		} else {
			&entry.destination
		};
		self.journal.writer_description = Some(format!(
			"{} / {} at {:?}",
			entry.task, entry.original.name, location
		));
		self.writer(true)
	}

	fn pending_writer_description(&self) -> Option<String> {
		self.journal.writer_description.clone()
	}

	fn writer(&mut self, pending: bool) -> Result<(), Problem> {
		self.verify()?;
		self.journal.writer_pending = pending;
		if !pending {
			self.journal.writer_description = None;
		}
		self.persist()
	}
	fn activate(&mut self, undo: bool) -> Result<(), Problem> {
		for (index, member) in self.journal.participants.iter().enumerate() {
			if let Some(selection) = &member.selection {
				selection.require_settled(&self.journal, index, undo)?;
			}
		}
		self.activate_state(undo)
	}
	fn cleanup(&mut self) -> Result<(), Problem> {
		self.cleanup_source()
	}
}
