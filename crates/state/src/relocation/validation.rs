//! Validate persisted relocation authority before trusting any stored path or index.
use super::{conflict, record::Journal};
use std::path::{Component, Path};
use workyard_core::Problem;

impl Journal {
	pub(super) fn validate(&self) -> Result<(), Problem> {
		if self.version != 2 || self.id.is_empty() || self.id.contains(['/', '\\']) {
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
		if self.copy && (old_root == new_root || old_state == new_state) {
			return Err(conflict(
				"Copy requires distinct workspace roots and distinct state directories.",
			));
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
			let target = new_root.join(&path);
			if self.copy
				&& (target.starts_with(&new_state)
					|| (new_state.starts_with(&target) && entry.kind != "directory"))
			{
				return Err(conflict(&format!(
					"Copied workspace entry overlaps destination state: {target:?}."
				)));
			}
			if old_root.join(&path).starts_with(&old_state)
				|| !content.insert(path)
				|| !matches!(entry.kind.as_str(), "file" | "directory" | "symlink")
				|| (entry.kind == "symlink") != entry.link.is_some()
			{
				return Err(conflict("Invalid workspace content inventory."));
			}
		}
		self.validate_membership(&old_root, &old_state)?;
		for (index, entry) in self.participants.iter().enumerate() {
			if let Some(selection) = &entry.selection {
				selection.validate(self, index)?;
			}
		}
		Ok(())
	}

	fn validate_membership(&self, old_root: &Path, old_state: &Path) -> Result<(), Problem> {
		for (name, task) in &self.tasks {
			crate::records::task::validate(task, name, old_root, old_state)?;
			crate::records::task::decode(task.clone())?;
			for participant in &task.participants {
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
		}

		let mut seen = std::collections::BTreeSet::new();
		for entry in &self.participants {
			let task = self
				.tasks
				.get(&entry.task)
				.ok_or_else(|| conflict("Unknown relocation task reference."))?;
			let destination = entry.destination.decode()?;
			if entry.index >= task.participants.len()
				|| !seen.insert((&entry.task, entry.index))
				|| !destination.is_absolute()
				|| destination.as_os_str().as_encoded_bytes().contains(&0)
			{
				return Err(conflict("Invalid relocation participant identity."));
			}
		}
		let expected_count = self
			.tasks
			.values()
			.map(|t| t.participants.len())
			.sum::<usize>();
		if seen.len() != expected_count {
			return Err(conflict("Missing relocation participant."));
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
