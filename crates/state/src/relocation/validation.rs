//! Validate persisted relocation authority before trusting any stored path or index.
use super::{conflict, record::Journal};
use std::path::{Component, Path};
use workyard_core::Problem;

impl Journal {
	pub(super) fn validate(&self) -> Result<(), Problem> {
		if self.version != 1 || self.id.is_empty() || self.id.contains(['/', '\\']) {
			return Err(conflict("Invalid relocation identity."));
		}
		let old_root = self.old_root.decode()?;
		let new_root = self.new_root.decode()?;
		let old_state = self.old_state.decode()?;
		let new_state = self.new_state.decode()?;
		for path in [&old_root, &new_root, &old_state, &new_state] {
			if !path.is_absolute()
				|| path.as_os_str().as_encoded_bytes().contains(&0)
				|| path
					.components()
					.any(|part| matches!(part, Component::ParentDir | Component::CurDir))
			{
				return Err(conflict(
					"Relocation anchors must be absolute native paths.",
				));
			}
		}
		super::admission::validate_placement(&old_root, &old_state, &new_root, &new_state)?;
		let mut files = std::collections::BTreeSet::new();
		for file in &self.files {
			let path = file.path.decode()?;
			relative(&path)?;
			if path == Path::new("workspace.lock")
				|| path == Path::new("relocation.json")
				|| !files.insert(path)
			{
				return Err(conflict("Invalid metadata transfer ownership."));
			}
		}
		for directory in &self.state_directories {
			relative(&directory.decode()?)?;
		}
		let mut content = std::collections::BTreeSet::new();
		for entry in &self.content {
			let path = entry.path.decode()?;
			relative(&path)?;
			if old_root.join(&path).starts_with(&old_state)
				|| !content.insert(path)
				|| !matches!(entry.kind.as_str(), "file" | "directory" | "symlink")
				|| (entry.kind == "symlink") != entry.link.is_some()
			{
				return Err(conflict("Invalid workspace content inventory."));
			}
		}
		for entry in &self.participants {
			crate::records::task::validate(&entry.task, &entry.task.task, &old_root, &old_state)?;
			crate::records::task::decode(entry.task.clone())?;
			for participant in &entry.task.participants {
				for path in [
					&participant.source,
					&participant.common_dir,
					&participant.destination,
				] {
					if path.decode()?.as_os_str().as_encoded_bytes().contains(&0) {
						return Err(conflict("Participant path contains NUL."));
					}
				}
			}
			if entry.index >= entry.task.participants.len()
				|| !entry.destination.decode()?.is_absolute()
				|| entry
					.destination
					.decode()?
					.as_os_str()
					.as_encoded_bytes()
					.contains(&0)
			{
				return Err(conflict("Invalid relocation participant identity."));
			}
		}
		Ok(())
	}
}

fn relative(path: &Path) -> Result<(), Problem> {
	if path.as_os_str().is_empty()
		|| path.as_os_str().as_encoded_bytes().contains(&0)
		|| path
			.components()
			.any(|part| !matches!(part, Component::Normal(_)))
	{
		return Err(conflict(
			"Relocation inventory contains an invalid relative path.",
		));
	}
	Ok(())
}
