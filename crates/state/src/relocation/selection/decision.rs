//! Compare live candidates, reuse an applicable decision and persist a new selection.
use super::{Relocation, Selection, conflict, endpoints, read_pointer, tree};
use crate::exists;
use workyard_core::{
	Control, Problem, RecoveryCandidates, RecoveryChoice, RegisteredCopy, RelocationParticipant,
};
use workyard_system::entry_identity;

impl Relocation {
	pub(in crate::relocation) fn candidates(
		&self,
		entry: &RelocationParticipant,
		undo: bool,
		reg: &RegisteredCopy,
		control: &Control,
	) -> Result<Option<RecoveryCandidates>, Problem> {
		let index = self.member_index(entry)?;
		let (dest, alternate) = endpoints(entry, undo);
		self.verify_candidates(index, entry, undo, reg)?;
		if let Some(selection) = &self.journal.participants[index].selection {
			let [stage, displaced] = selection.temporary_paths(&self.journal, index)?;
			if selection.undo == undo && selection.registered()? == reg.path {
				// Once publication starts, its named endpoints and directory identities
				// let retries finish the approved installation across either rename.
				if exists(&stage)? || exists(&displaced)? {
					return Ok(None);
				}
			} else if exists(&stage)? || exists(&displaced)? {
				return Err(conflict(
					"Finish the pending participant installation before changing recovery direction.",
				));
			}
		}
		let occupied = exists(dest)?;
		let first = if occupied { dest } else { alternate };
		let first_digest = if exists(first)? {
			Some(tree::fingerprint(first, control)?)
		} else {
			None
		};
		let registered_digest = tree::fingerprint(&reg.path, control)?;
		if let Some(s) = &self.journal.participants[index].selection
			&& s.undo == undo
			&& s.registered()? == reg.path
			&& s.registered_digest == registered_digest
		{
			if s.stage_id.is_some()
				&& occupied && s.stage_id == Some(entry_identity(dest)?)
				&& first_digest.as_ref() == Some(&s.selected_digest)
			{
				return Ok(None);
			}
			if s.occupied == occupied && first_digest == s.first_digest {
				return Ok(None);
			}
		}
		Ok(Some(RecoveryCandidates {
			choice: RecoveryChoice {
				participant: entry.original.name.clone(),
				destination: dest.into(),
				first: first.into(),
				registered: reg.path.clone(),
				occupied,
			},
			first_digest,
			registered_digest,
		}))
	}

	pub(in crate::relocation) fn select(
		&mut self,
		entry: &RelocationParticipant,
		undo: bool,
		reg: &RegisteredCopy,
		basis: &RecoveryCandidates,
		use_registered: bool,
	) -> Result<(), Problem> {
		let index = self.member_index(entry)?;
		let choice = &basis.choice;
		let selected = if use_registered {
			&basis.registered_digest
		} else {
			basis
				.first_digest
				.as_ref()
				.ok_or_else(|| conflict("Selected source disappeared."))?
		};
		let occupied = choice.occupied;
		let selection = Selection {
			registered: reg.path.as_path().into(),
			administration: reg.administration.as_path().into(),
			registered_id: entry_identity(&reg.path)?,
			undo,
			use_registered,
			occupied,
			selected_digest: selected.clone(),
			first_digest: basis.first_digest.clone(),
			registered_digest: basis.registered_digest.clone(),
			retired_entries: Vec::new(),
			displaced_pointer: if occupied {
				Some(read_pointer(&choice.destination)?)
			} else {
				None
			},
			retiring: false,
			displaced_id: if occupied {
				Some(entry_identity(&choice.destination)?)
			} else {
				None
			},
			stage_id: None,
			installed: false,
		};
		for path in selection.temporary_paths(&self.journal, index)? {
			if exists(&path)? {
				return Err(conflict("Participant staging path is already occupied."));
			}
		}
		self.journal.participants[index].selection = Some(selection);
		self.persist()
	}
}
