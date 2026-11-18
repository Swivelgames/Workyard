//! State-last publication and ownership-checked cleanup.
use super::{Relocation, conflict, metadata, record::Journal};
use crate::{
	documents::{publish, publish_bytes, read_versioned},
	records::WorkspaceRecord,
};
use std::{collections::BTreeSet, fs, path::Path};
use workyard_core::{Problem, RelocationState};

impl Relocation {
	pub(super) fn activate_state(&mut self, undo: bool) -> Result<(), Problem> {
		self.verify()?;
		let source = self.old_state()?;
		let destination = if undo {
			source.clone()
		} else {
			self.new_state()?
		};
		let root = if undo {
			self.old_root()?
		} else {
			self.new_root()?
		};
		self.validate_state_contents(&source, true)?;
		self.validate_state_contents(&destination, undo)?;
		for file in &self.journal.files {
			let relative = file.path.decode()?;
			if relative == Path::new("workspace.json") {
				continue;
			}
			let path = destination.join(relative);
			if let Some(parent) = path.parent() {
				fs::create_dir_all(parent)
					.map_err(|error| Problem::io("Create state transfer directory", error))?;
			}
			let bytes = if undo { &file.original } else { &file.target };
			publish_bytes(&path, bytes)?;
		}
		for directory in &self.journal.state_directories {
			fs::create_dir_all(destination.join(directory.decode()?))
				.map_err(|error| Problem::io("Preserve state directory", error))?;
		}
		for name in ["tasks", "evidence", "locks"] {
			fs::create_dir_all(destination.join(name))
				.map_err(|error| Problem::io("Create empty state directory", error))?;
		}
		for file in &self.journal.files {
			let relative = file.path.decode()?;
			if relative == Path::new("workspace.json") {
				continue;
			}
			let expected = if undo { &file.original } else { &file.target };
			if &fs::read(destination.join(relative))
				.map_err(|error| Problem::io("Verify transferred state", error))?
				!= expected
			{
				return Err(conflict("Copied state differs before activation."));
			}
		}
		self.verify()?;
		if source != destination {
			self.validate_state_contents(&source, true)?;
		}
		// The target journal precedes the authority marker. Source may remain pending.
		self.journal.undone = undo;
		self.journal.activation_pending = true;
		self.persist()?;
		publish(&destination.join("relocation.json"), &self.journal)?;
		self.binding(&destination, &root, "active")?;
		self.journal.activated = true;
		self.persist()
	}

	fn validate_state_contents(&self, directory: &Path, undo: bool) -> Result<(), Problem> {
		let allowed: BTreeSet<_> = self
			.journal
			.files
			.iter()
			.map(|file| file.path.decode())
			.collect::<Result<_, _>>()?;
		for path in metadata::directories(directory)? {
			if !self.journal.state_directories.contains(&path) {
				return Err(conflict(
					"Unexpected metadata directory; cleanup preserves authored entries.",
				));
			}
		}
		for file in metadata::snapshot(directory)? {
			let path = file.path.decode()?;
			if path == Path::new("relocation.json") || path == Path::new("workspace.json") {
				continue;
			}
			if !allowed.contains(&path) {
				return Err(conflict(
					"Unexpected metadata at relocation destination; no files were overwritten.",
				));
			}
			let expected = self
				.journal
				.files
				.iter()
				.find(|entry| entry.path == file.path)
				.expect("allowed path");
			if file.original != expected.original && file.original != expected.target {
				return Err(conflict(
					"Relocation metadata was edited; preserve it before continuing.",
				));
			}
		}
		if crate::exists(&directory.join("relocation.json"))? {
			let paired: Journal = read_versioned(&directory.join("relocation.json"))?;
			self.pair(&paired)?;
		}
		let manifest = directory.join("workspace.json");
		if self.journal.copy && directory == self.old_state()? && !crate::exists(&manifest)? {
			return Err(conflict("Copy source workspace binding is missing."));
		}
		if crate::exists(&manifest)? {
			let binding = WorkspaceRecord::read(&directory.join("workspace.json"))?;
			let source = directory == self.old_state()?;
			if self.journal.copy {
				self.verify_copy_binding(directory, source, &binding)?;
			} else if !source && binding.relocation.as_deref() != Some(&self.journal.id) {
				return Err(conflict(
					"Destination workspace belongs to another operation.",
				));
			}
			if !undo
				&& directory != self.old_state()?
				&& binding.authority.as_deref() == Some("active")
			{
				return Err(conflict(
					"Destination is already active; recovery must only clean source metadata.",
				));
			}
		}
		Ok(())
	}

	pub(super) fn pair(&self, other: &Journal) -> Result<(), Problem> {
		if other.id != self.journal.id
			|| other.copy != self.journal.copy
			|| other.old_root != self.journal.old_root
			|| other.new_root != self.journal.new_root
			|| other.old_state != self.journal.old_state
			|| other.new_state != self.journal.new_state
		{
			return Err(conflict(
				"Relocation operation identity or original anchors disagree.",
			));
		}
		Ok(())
	}

	pub(super) fn cleanup_source(&mut self) -> Result<(), Problem> {
		self.verify()?;
		let source = self.old_state()?;
		let destination = self.new_state()?;
		let active = if self.journal.undone {
			&source
		} else {
			&destination
		};
		let active_root = self.active_root()?;
		let active_binding = WorkspaceRecord::read(&active.join("workspace.json"))?;
		let active_journal: Journal = read_versioned(&active.join("relocation.json"))?;
		self.pair(&active_journal)?;
		if active_binding.relocation.as_deref() != Some(&self.journal.id)
			|| active_binding.state_dir.decode()? != *active
			|| active_binding.tasks_root.decode()? != active_root
			|| active_binding.authority.as_deref() != Some("active")
		{
			return Err(conflict(
				"Active destination pairing could not be verified; source is preserved.",
			));
		}
		let obsolete = if source == destination {
			None
		} else if self.journal.undone {
			Some((&destination, false))
		} else {
			Some((&source, true))
		};
		if let Some((directory, is_source)) = obsolete {
			if self.journal.copy {
				self.retain_snapshot(directory, is_source)?;
			} else if crate::exists(directory)? {
				self.validate_state_contents(directory, true)?;
				if is_source {
					self.binding(directory, &self.old_root()?, "inactive")?;
				}
				self.remove_metadata(directory, is_source)?;
			}
		}

		self.journal.cleanup_complete = true;
		self.persist()?;
		// A settled journal is no longer mutation admission. Keep paired history in
		// the active manifest, but permit a later independent relocation.
		fs::remove_file(active.join("relocation.json"))
			.map_err(|error| Problem::io("Remove completed relocation intent", error))?;
		Ok(())
	}

	fn remove_metadata(&mut self, directory: &Path, source: bool) -> Result<(), Problem> {
		for file in &self.journal.files {
			let relative = file.path.decode()?;
			if relative == Path::new("workspace.json") {
				continue;
			}
			let path = directory.join(relative);
			match fs::read(&path) {
				Ok(bytes) if bytes == file.original || bytes == file.target => {
					fs::remove_file(&path)
						.map_err(|error| Problem::io("Remove owned source metadata", error))?
				}
				Ok(_) => {
					return Err(conflict(
						"Source metadata changed after activation; cleanup stopped.",
					));
				}
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
				Err(error) => return Err(Problem::io("Verify old metadata before cleanup", error)),
			}
		}
		let mut directories = self
			.journal
			.state_directories
			.iter()
			.map(|path| path.decode())
			.collect::<Result<Vec<_>, _>>()?;
		directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
		for relative in directories {
			match fs::remove_dir(directory.join(relative)) {
				Ok(()) => {}
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
				Err(error) => {
					return Err(Problem::io("Remove owned empty metadata directory", error));
				}
			}
		}
		let gate = if source {
			&mut self.source_gate
		} else {
			&mut self.destination_gate
		};
		if let Some(held) = gate.as_ref() {
			held.verify()?;
		}
		// Keep the inactive manifest until the named old gate is gone. If native
		// deletion refuses an open gate, leave the marker and report cleanup failure.
		fs::remove_file(directory.join("workspace.lock")).map_err(|error| {
			Problem::io("Remove old workspace gate; placement is complete", error)
		})?;
		gate.take();
		let journal_path = directory.join("relocation.json");
		if crate::exists(&journal_path)? {
			let paired: Journal = read_versioned(&journal_path)?;
			self.pair(&paired)?;
			fs::remove_file(&journal_path)
				.map_err(|error| Problem::io("Remove paired source intent", error))?;
		}
		let manifest = directory.join("workspace.json");
		if crate::exists(&manifest)? {
			let binding = WorkspaceRecord::read(&manifest)?;
			if binding.relocation.as_deref() != Some(&self.journal.id)
				|| binding.authority.as_deref() == Some("active")
			{
				return Err(conflict("Source authority changed during cleanup."));
			}
			if crate::exists(&directory.join("workspace.lock"))? {
				return Err(conflict(
					"A new source gate appeared; cleanup preserved it.",
				));
			}
			fs::remove_file(manifest)
				.map_err(|error| Problem::io("Remove inactive source marker", error))?;
		}
		fs::remove_dir(directory).map_err(|error| {
			Problem::io(
				"Remove empty old state directory; placement is complete",
				error,
			)
		})?;
		Ok(())
	}
}
