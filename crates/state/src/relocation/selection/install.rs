//! Install an approved tree using recoverable no-replace directory renames.
use super::{Relocation, Selection, cleanup, conflict, endpoints, tree};
use crate::{canonical_future, exists, relocation::content::permissions};
use std::path::Path;
use workyard_core::{Control, Problem, RegisteredCopy, RelocationParticipant, RelocationState};
use workyard_system::{entry_identity, rename_no_replace};

impl Relocation {
	pub(in crate::relocation) fn install(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		reg: &RegisteredCopy,
		control: &Control,
	) -> Result<(), Problem> {
		self.verify()?;
		let index = self.member_index(entry)?;
		let mut selection = self.journal.participants[index]
			.selection
			.clone()
			.ok_or_else(|| conflict("Missing participant selection."))?;
		let (dest, alternate) = endpoints(entry, undo);
		let [stage, displaced] = selection.temporary_paths(&self.journal, index)?;
		if selection.registered()? != reg.path
			|| selection.undo != undo
			|| canonical_future(dest)? != dest
		{
			return Err(conflict("Participant recovery location changed."));
		}
		let source = if selection.use_registered {
			reg.path.as_path()
		} else if selection.occupied {
			dest
		} else {
			alternate
		};

		// Keeping the occupied endpoint only changes its Git registration.
		if source == dest {
			verify_contents(dest, &selection.selected_digest, control)?;
			selection.installed = true;
			return self.save_selection(index, &selection);
		}
		let published = exists(dest)? && selection.stage_id == Some(entry_identity(dest)?);
		if !published {
			verify_contents(source, &selection.selected_digest, control)?;
			if !exists(&displaced)? {
				selection.verify_old_destination(dest, control)?;
			}
			self.stage_selection(
				index,
				&mut selection,
				source,
				&stage,
				&reg.administration,
				control,
			)?;
			control.check()?;
			verify_contents(source, &selection.selected_digest, control)?;
			if selection.occupied && !exists(&displaced)? {
				selection.verify_old_destination(dest, control)?;
				selection.retired_entries = cleanup::retired_inventory(dest, control)?;
				self.save_selection(index, &selection)?;
				move_directory(dest, &displaced)?;
				control.check()?;
			}
			cleanup::verify_pointer(&stage, &reg.administration)?;
			move_directory(&stage, dest)?;
		}
		verify_contents(dest, &selection.selected_digest, control)?;
		cleanup::verify_pointer(dest, &reg.administration)?;
		self.finish_retired(index, &mut selection, &displaced, control)?;
		selection.installed = true;
		self.save_selection(index, &selection)
	}

	pub(super) fn finish_retired(
		&mut self,
		index: usize,
		selection: &mut Selection,
		displaced: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		if exists(displaced)? {
			if Some(entry_identity(displaced)?) != selection.displaced_id {
				return Err(conflict("Displaced directory changed before cleanup."));
			}
			if !selection.retiring {
				selection.verify_old_destination(displaced, control)?;
				selection.retiring = true;
				self.save_selection(index, selection)?;
			}
			cleanup::discard_retired(displaced, &selection.retired_entries, control)?;
		}
		Ok(())
	}

	fn stage_selection(
		&mut self,
		index: usize,
		selection: &mut Selection,
		source: &Path,
		stage: &Path,
		admin: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		if exists(stage)? {
			if Some(entry_identity(stage)?) != selection.stage_id {
				return Err(conflict("Recovery staging directory changed."));
			}
			if tree::fingerprint(stage, control)? == selection.selected_digest
				&& exists(&stage.join(".git"))?
			{
				return cleanup::verify_pointer(stage, admin)
					.map_err(|_| super::resume::stage_guidance(stage));
			}
			// A stopped copy may leave a private, incomplete stage. The source is
			// intact and still has the approved digest.
			cleanup::discard_partial(stage, source, admin, control).map_err(|p| {
				if p.code == workyard_core::ProblemCode::Cancelled {
					p
				} else {
					super::resume::stage_guidance(stage)
				}
			})?;
		}
		permissions::private_directories()
			.recursive(false)
			.create(stage)
			.map_err(|e| Problem::io("Create private participant stage", e))?;
		selection.stage_id = Some(entry_identity(stage)?);
		self.save_selection(index, selection)?;
		tree::populate(source, stage, admin, control)?;
		verify_contents(stage, &selection.selected_digest, control)
	}
}

impl Selection {
	pub(super) fn verify_old_destination(
		&self,
		path: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		if !self.occupied && !exists(path)? {
			return Ok(());
		}
		if !exists(path)? || self.displaced_id != Some(entry_identity(path)?) {
			return Err(conflict("Recovery destination changed after selection."));
		}
		if self.displaced_pointer.as_deref() != Some(super::read_pointer(path)?.as_slice()) {
			return Err(conflict("Displaced participant Git pointer changed."));
		}
		let expected = self
			.first_digest
			.as_ref()
			.ok_or_else(|| conflict("Missing destination content evidence."))?;
		verify_contents(path, expected, control)
	}
}
fn verify_contents(path: &Path, digest: &str, control: &Control) -> Result<(), Problem> {
	if tree::fingerprint(path, control)? != digest {
		return Err(conflict(&format!(
			"Participant files changed at {path:?}. Run recover again to inspect the current choices."
		)));
	}
	Ok(())
}
fn move_directory(from: &Path, to: &Path) -> Result<(), Problem> {
	rename_no_replace(from, to)
		.map_err(|e| Problem::io(format!("Publish participant directory at {to:?}"), e))
}
