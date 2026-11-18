//! Fresh admission validates all anchors and inventories under the source gate.
use super::{
	Relocation, conflict, content, metadata,
	record::{Journal, Participant},
};
use crate::{Lock, LockMode, canonical_future, records::WorkspaceRecord};
use std::{fs, path::Path};
use workyard_core::{Problem, ProblemCode};

impl Relocation {
	pub fn open(
		old_state: &Path,
		requested_root: Option<&Path>,
		new_root: &Path,
		new_state: Option<&Path>,
	) -> Result<Self, Problem> {
		Self::open_control(
			old_state,
			requested_root,
			new_root,
			new_state,
			&workyard_core::Control::default(),
		)
	}

	pub fn open_control(
		old_state: &Path,
		requested_root: Option<&Path>,
		new_root: &Path,
		new_state: Option<&Path>,
		control: &workyard_core::Control,
	) -> Result<Self, Problem> {
		Self::open_mode(
			old_state,
			requested_root,
			new_root,
			new_state,
			control,
			false,
		)
	}

	pub fn open_copy_control(
		old_state: &Path,
		requested_root: Option<&Path>,
		new_root: &Path,
		new_state: Option<&Path>,
		control: &workyard_core::Control,
	) -> Result<Self, Problem> {
		Self::open_mode(
			old_state,
			requested_root,
			new_root,
			new_state,
			control,
			true,
		)
	}

	fn open_mode(
		old_state: &Path,
		requested_root: Option<&Path>,
		new_root: &Path,
		new_state: Option<&Path>,
		control: &workyard_core::Control,
		copy: bool,
	) -> Result<Self, Problem> {
		let old_state = canonical_future(old_state)?;
		for name in ["workspace.json", "workspace.lock"] {
			if !old_state
				.join(name)
				.try_exists()
				.map_err(|error| Problem::io("Inspect source state", error))?
			{
				return Err(conflict(
					"Authoritative source state or its gate is missing; recovery cannot initialize it.",
				));
			}
		}
		let source_gate =
			Lock::acquire_mode(&old_state.join("workspace.lock"), LockMode::Exclusive)?;
		let record = WorkspaceRecord::read(&old_state.join("workspace.json"))?;
		super::snapshot::require_live(&old_state, &record)?;
		let old_root = record.tasks_root.decode()?;
		if record.state_dir.decode()? != old_state
			|| record
				.authority
				.as_deref()
				.is_some_and(|status| status != "active")
			|| old_state
				.join("relocation.json")
				.try_exists()
				.map_err(|error| Problem::io("Inspect relocation intent", error))?
		{
			return Err(conflict(
				"Source state has relocation evidence; inspect it and use recover.",
			));
		}
		if let Some(requested) = requested_root
			&& canonical_future(requested)? != old_root
		{
			return Err(conflict(
				"Explicit source root differs from its state association.",
			));
		}
		let new_root = canonical_future(new_root)?;
		let new_state = canonical_future(&new_state.map_or_else(
			|| {
				if copy || old_state == old_root.join(".workyard") {
					new_root.join(".workyard")
				} else {
					old_state.clone()
				}
			},
			Path::to_path_buf,
		))?;
		if copy && (new_root == old_root || new_state == old_state) {
			return Err(conflict(
				"Copy requires distinct workspace roots and distinct state directories.",
			));
		}
		validate_placement(&old_root, &old_state, &new_root, &new_state)?;
		if new_root != old_root {
			empty(&new_root)?;
		}
		if new_state != old_state {
			empty(&new_state)?;
		}
		let tasks = metadata::tasks(&old_state, &old_root)?;
		let mut participants = Vec::new();
		for task in &tasks {
			for (index, participant) in task.participants.iter().enumerate() {
				for backing in [&participant.source, &participant.common_dir] {
					if new_root != old_root && backing.decode()?.starts_with(&old_root) {
						return Err(conflict(
							"Backing repositories inside the moved root cannot be moved by workspace relocation.",
						));
					}
				}
				let source = participant.destination.decode()?;
				let destination = source
					.strip_prefix(&old_root)
					.map_or_else(|_| source.clone(), |relative| new_root.join(relative));
				participants.push(Participant {
					task: task.task.clone(),
					index,
					absent_at_admission: false,
					selection: None,
					recovered_source: None,
					recovered_destination: None,
					backup_endpoints: Vec::new(),
					destination: destination.as_path().into(),
				});
			}
		}
		let id = format!(
			"{}-{}",
			std::process::id(),
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map_err(|error| Problem::new(ProblemCode::IoError, error.to_string()))?
				.as_nanos()
		);
		let journal = Journal {
			version: 2,
			id,
			copy,
			copy_verified: false,
			old_root: old_root.as_path().into(),
			new_root: new_root.as_path().into(),
			old_state: old_state.as_path().into(),
			new_state: new_state.as_path().into(),
			tasks: tasks
				.into_iter()
				.map(|task| (task.task.clone(), task))
				.collect(),
			participants,
			files: metadata::snapshot(&old_state)?,
			state_directories: metadata::directories(&old_state)?,
			content: if old_root == new_root {
				Vec::new()
			} else {
				content::inventory(&old_root, &old_state, control)?
			},
			writer_pending: false,
			writer_description: None,
			activated: false,
			activation_pending: false,
			undone: false,
			cleanup_complete: false,
		};
		let mut relocation = Self {
			journal,
			source_gate: Some(source_gate),
			destination_gate: None,
			started: false,
		};
		relocation.journal.validate()?;
		metadata::transform(&mut relocation.journal)?;
		Ok(relocation)
	}
}

pub(super) fn empty(path: &Path) -> Result<(), Problem> {
	match fs::read_dir(path) {
		Ok(mut entries) => {
			if entries.next().is_some() {
				Err(conflict(&format!(
					"Relocation target is occupied: {path:?}."
				)))
			} else {
				Ok(())
			}
		}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
		Err(error) => Err(Problem::io("Inspect relocation target", error)),
	}
}

pub(super) fn validate_placement(
	old: &Path,
	old_state: &Path,
	new: &Path,
	new_state: &Path,
) -> Result<(), Problem> {
	if old != new && (old.starts_with(new) || new.starts_with(old)) {
		return Err(conflict("Source and destination workspace roots overlap."));
	}
	if old_state != new_state
		&& (old_state.starts_with(new_state) || new_state.starts_with(old_state))
	{
		return Err(conflict(
			"Source and destination state directories overlap.",
		));
	}
	if old != new
		&& ((new_state != old_state && new_state.starts_with(old)) || old_state.starts_with(new))
	{
		return Err(conflict(
			"State placement crosses the other workspace root.",
		));
	}
	if old_state == old
		|| new_state == new
		|| old.starts_with(old_state)
		|| new.starts_with(new_state)
	{
		return Err(conflict("State cannot contain the workspace root."));
	}
	Ok(())
}
