//! Reconcile previously approved file choices with current Git and filesystem facts.
use super::{
	Relocation, Selection, cleanup, conflict, endpoints, read_directory, read_pointer, tree,
};
use crate::{canonical_future, exists};
use std::{fs, path::Path};
use workyard_core::{Control, Problem, RegisteredCopy, RelocationParticipant};
use workyard_system::{directory_identity, entry_identity, rename_no_replace};

impl Relocation {
	/// Git already points at the requested endpoint. An unpublished decision
	/// from a previous attempt must not make the live worktree unusable.
	pub(in crate::relocation) fn accept_endpoint(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		control: &Control,
	) -> Result<(), Problem> {
		let index = self.member_index(entry)?;
		let Some(mut selection) = self.journal.participants[index].selection.clone() else {
			return Ok(());
		};
		let dest = endpoints(entry, undo).0;
		if canonical_future(dest)? != dest {
			return Err(conflict(
				"Registered endpoint traverses a redirected directory.",
			));
		}
		self.verify_directory_identities(index, entry)?;
		let [stage, displaced] = selection.temporary_paths(&self.journal, index)?;
		if selection.installed && selection.undo == undo && !exists(&stage)? && !exists(&displaced)?
		{
			return selection.require_settled(&self.journal, index, undo);
		}
		let reg = RegisteredCopy {
			path: selection.registered()?,
			bound_at: dest.to_owned(),
			administration: selection.administration.decode()?,
		};
		if Some(directory_identity(&reg.administration)?) != entry.original.worktree_identity {
			return Err(conflict("Recovery administrative identity changed."));
		}
		self.settle_files(index, entry, &mut selection, &reg, control)?;
		selection.undo = undo;
		selection.occupied = true;
		selection.use_registered = false;
		selection.stage_id = None;
		selection.displaced_id = Some(entry_identity(dest)?);
		selection.displaced_pointer = Some(read_pointer(dest)?);
		selection.selected_digest = tree::fingerprint(dest, control)?;
		selection.first_digest = Some(selection.selected_digest.clone());
		selection.installed = true;
		self.save_selection(index, &selection)
	}

	pub(in crate::relocation) fn settle_direction(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		reg: &RegisteredCopy,
		control: &Control,
	) -> Result<(), Problem> {
		let index = self.member_index(entry)?;
		let Some(mut selection) = self.journal.participants[index].selection.clone() else {
			return Ok(());
		};
		let [stage, displaced] = selection.temporary_paths(&self.journal, index)?;
		if !exists(&stage)? && !exists(&displaced)? {
			// The backup rename may have reached disk just before its journal update.
			// Settle that handoff before a new choice can supersede this selection.
			let previous_endpoint = endpoints(entry, selection.undo).0;
			if let Some(number) = selection.backup_number
				&& exists(previous_endpoint)?
				&& selection.stage_id == Some(entry_identity(previous_endpoint)?)
			{
				self.retain_backup(index, &selection, number, &displaced)?;
			}
			return Ok(());
		}
		if canonical_future(stage.parent().expect("staging parent"))?
			!= stage.parent().expect("staging parent")
		{
			return Err(conflict("Recovery staging parent changed."));
		}
		if exists(&stage)? && Some(entry_identity(&stage)?) != selection.stage_id {
			remove_unpublished_empty(&stage)?;
			selection.stage_id = None;
			self.save_selection(index, &selection)?;
		}
		if selection.registered()? != reg.path {
			return Err(conflict(
				"Git registration changed during participant publication.",
			));
		}
		let (prior_dest, alternate) = endpoints(entry, selection.undo);
		let source = if selection.use_registered {
			reg.path.as_path()
		} else {
			alternate
		};
		if selection.undo == undo {
			let selected_unchanged =
				exists(source)? && tree::fingerprint(source, control)? == selection.selected_digest;
			// While the original directory is displaced, compare against its one
			// live name. Changes there cannot authorize erasure.
			let previous = if exists(&displaced)? {
				displaced.as_path()
			} else if selection.occupied {
				prior_dest
			} else {
				alternate
			};
			let first_unchanged = if exists(previous)? {
				Some(tree::fingerprint(previous, control)?) == selection.first_digest
			} else {
				selection.first_digest.is_none()
			};
			if selected_unchanged && first_unchanged {
				return Ok(());
			}
		}
		self.settle_files(index, entry, &mut selection, reg, control)?;
		// New observations will either produce a new choice or converge automatically.
		self.journal.participants[index].selection = None;
		self.persist()
	}

	fn settle_files(
		&mut self,
		index: usize,
		entry: &RelocationParticipant,
		selection: &mut Selection,
		reg: &RegisteredCopy,
		control: &Control,
	) -> Result<(), Problem> {
		let [stage, displaced] = selection.temporary_paths(&self.journal, index)?;
		let (dest, alternate) = endpoints(entry, selection.undo);
		if canonical_future(dest)? != dest {
			return Err(conflict("Recovery endpoint traverses a redirected path."));
		}
		if exists(dest)? && selection.stage_id == Some(entry_identity(dest)?) {
			// The selected files are published. Finish the handoff of the displaced files.
			if exists(&displaced)? {
				if tree::fingerprint(dest, control)? != selection.selected_digest {
					return Err(conflict(
						"Recovered participant changed before displaced file cleanup.",
					));
				}
				cleanup::verify_pointer(dest, &reg.administration)?;
			}
			return self.finish_displaced(index, selection, &displaced, control);
		}
		if exists(&displaced)? {
			selection.verify_old_destination(&displaced, control)?;
			rename_no_replace(&displaced, dest).map_err(|e| {
				Problem::io(
					"Restore displaced participant before reconsidering files",
					e,
				)
			})?;
		}
		if !exists(&stage)? {
			return Ok(());
		}
		if Some(entry_identity(&stage)?) != selection.stage_id {
			return remove_unpublished_empty(&stage);
		}
		if tree::fingerprint(&stage, control)? == selection.selected_digest
			&& exists(&stage.join(".git"))?
		{
			cleanup::verify_pointer(&stage, &reg.administration)?;
			let evidence = cleanup::retired_inventory(&stage, control)?;
			if tree::fingerprint(&stage, control)? != selection.selected_digest {
				return Err(stage_guidance(&stage));
			}
			cleanup::verify_pointer(&stage, &reg.administration)?;
			return cleanup::discard_retired(&stage, &evidence, control);
		}
		let source = if selection.use_registered {
			reg.path.as_path()
		} else {
			alternate
		};
		cleanup::discard_partial(&stage, source, &reg.administration, control).map_err(|p| {
			if p.code == workyard_core::ProblemCode::Cancelled {
				p
			} else {
				stage_guidance(&stage)
			}
		})
	}
}

/// Production may stop after mkdir and before publishing the identity. remove_dir
/// itself verifies emptiness at the point of removal, including concurrent arrivals.
fn remove_unpublished_empty(path: &Path) -> Result<(), Problem> {
	if read_directory(path).is_err() {
		return Err(stage_guidance(path));
	}
	fs::remove_dir(path).map_err(|_| stage_guidance(path))
}

pub(super) fn stage_guidance(path: &Path) -> Problem {
	conflict(&format!("Recovery cannot account for files at {path:?}.")).remedy(format!(
		"Move {path:?} outside the workspace to keep its contents, then run workyard recover again."
	))
}
