//! Typed placement conversion; historical Git paths remain historical evidence.
use super::record::{Journal, StateFile};
use crate::{
	TaskRecord, documents::read_supported, evidence::EffectRecord, records::task::validate,
	revision::Revision,
};
use std::{fs, path::Path};
use workyard_core::{Problem, ProblemCode};

pub(super) fn snapshot(state: &Path) -> Result<Vec<StateFile>, Problem> {
	let mut files = Vec::new();
	visit(state, state, &mut files)?;
	Ok(files)
}

fn visit(root: &Path, directory: &Path, files: &mut Vec<StateFile>) -> Result<(), Problem> {
	for entry in
		fs::read_dir(directory).map_err(|error| Problem::io("Read source metadata", error))?
	{
		let entry = entry.map_err(|error| Problem::io("Read metadata entry", error))?;
		let path = entry.path();
		let relative = path.strip_prefix(root).expect("state descendant");
		if relative == Path::new("workspace.lock") {
			continue;
		}
		let kind = entry
			.file_type()
			.map_err(|error| Problem::io("Inspect metadata entry", error))?;
		if kind.is_dir() {
			visit(root, &path, files)?;
		} else if kind.is_file() {
			let original =
				fs::read(&path).map_err(|error| Problem::io("Read metadata bytes", error))?;
			files.push(StateFile {
				path: relative.into(),
				target: original.clone(),
				original,
			});
		} else {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"State contains a symlink or special file; relocate requires ordinary metadata files.",
			));
		}
	}
	Ok(())
}

pub(super) fn transform(journal: &mut Journal) -> Result<(), Problem> {
	let old_root = journal.old_root.decode()?;
	let old_state = journal.old_state.decode()?;
	let new_root = journal.new_root.decode()?;
	let new_state = journal.new_state.decode()?;
	for file in &mut journal.files {
		let path = file.path.decode()?;
		let history = path.starts_with("history");
		let update = |task: &mut TaskRecord| -> Result<(), Problem> {
			validate(task, &task.task, &old_root, &old_state)?;
			task.tasks_root = new_root.as_path().into();
			task.state_dir = new_state.as_path().into();
			if history || task.lifecycle == "retired" {
				return Ok(());
			}
			for participant in &mut task.participants {
				let known = journal.participants.iter().find(|entry| {
					entry.task == task.task
						&& journal.tasks[&entry.task].generation == task.generation
						&& entry.original(&journal.tasks).backing_identity
							== participant.backing_identity
				});
				if let Some(known) = known {
					participant.destination = known.destination.clone();
					let observed = known.original(&journal.tasks);
					participant.worktree_identity = observed.worktree_identity.clone();
					participant.source = observed.source.clone();
					participant.common_dir = observed.common_dir.clone();
				} else if let Ok(relative) =
					participant.destination.decode()?.strip_prefix(&old_root)
				{
					participant.destination = new_root.join(relative).as_path().into();
				}
			}
			Ok(())
		};
		if (path.starts_with("tasks") || path.starts_with("evidence") || history)
			&& path
				.extension()
				.is_some_and(|extension| extension == "json")
		{
			let mut task: TaskRecord = decode(&file.original)?;
			update(&mut task)?;
			file.target = encode(&task)?;
		} else if path.starts_with("evidence")
			&& path
				.extension()
				.is_some_and(|extension| extension == "effects")
		{
			let mut effects: EffectRecord = decode(&file.original)?;
			update(&mut effects.task)?;
			file.target = encode(&effects)?;
		} else if path.starts_with("evidence")
			&& path
				.extension()
				.is_some_and(|extension| extension == "revision")
		{
			let mut revision: Revision = decode(&file.original)?;
			update(&mut revision.anchor)?;
			update(&mut revision.current.task)?;
			let mut request = revision.request.decode()?;
			for repository in &mut request.repositories {
				if let Ok(relative) = repository.path.strip_prefix(&old_root) {
					repository.path = new_root.join(relative);
				}
			}
			revision.request = crate::revision::RequestRecord::from(&request);
			file.target = encode(&revision)?;
		}
	}
	Ok(())
}

pub(super) fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, Problem> {
	let mut bytes = Vec::new();
	let mut serializer = serde_json::Serializer::with_formatter(
		&mut bytes,
		serde_json::ser::PrettyFormatter::with_indent(b"\t"),
	);
	value
		.serialize(&mut serializer)
		.map_err(|error| Problem::new(ProblemCode::SerializationFailed, error.to_string()))?;
	bytes.push(b'\n');
	Ok(bytes)
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Problem> {
	serde_json::from_slice(bytes)
		.map_err(|error| Problem::new(ProblemCode::DamagedState, error.to_string()))
}

pub(super) fn tasks(state: &Path, root: &Path) -> Result<Vec<TaskRecord>, Problem> {
	let mut tasks = Vec::new();
	for entry in fs::read_dir(state.join("evidence"))
		.map_err(|error| Problem::io("Read admission evidence", error))?
	{
		let path = entry
			.map_err(|error| Problem::io("Read admission entry", error))?
			.path();
		if path.extension().is_none_or(|extension| extension != "json") {
			continue;
		}
		let intent: TaskRecord = read_supported(&path, &[1, 2, 3, 4])?;
		validate(&intent, &intent.task, root, state)?;
		if let Some(revision) = crate::revision::read(&path, &intent, root, state)? {
			if revision.current.pending.is_some() {
				return Err(unsettled());
			}
			tasks.push(revision.current.task);
			continue;
		}
		let effects = path.with_extension("effects");
		if crate::exists(&effects)? {
			let effect: EffectRecord = read_supported(&effects, &[1])?;
			if effect.pending.is_some() {
				return Err(unsettled());
			}
			validate(&effect.task, &intent.task, root, state)?;
			crate::records::task::match_intent(&intent, &effect.task)?;
			tasks.push(effect.task);
		} else {
			let published = state
				.join("tasks")
				.join(path.file_name().expect("task filename"));
			let saved: TaskRecord = read_supported(&published, &[1, 2, 3, 4])?;
			validate(&saved, &intent.task, root, state)?;
			crate::records::task::match_intent(&intent, &saved)?;
			if saved
				.participants
				.iter()
				.any(|participant| matches!(participant.phase.as_str(), "creating" | "pulling"))
			{
				return Err(unsettled());
			}
			tasks.push(saved);
		}
	}
	Ok(tasks)
}

fn unsettled() -> Problem {
	Problem::new(
		ProblemCode::ResourceBusy,
		"Task evidence has an unsettled managed writer. A free gate does not prove it stopped.",
	)
}

/// Directory inventory also protects authored empty directories during cleanup.
pub(super) fn directories(state: &Path) -> Result<Vec<crate::path::StoredPath>, Problem> {
	fn visit(
		root: &Path,
		directory: &Path,
		result: &mut Vec<crate::path::StoredPath>,
	) -> Result<(), Problem> {
		for entry in fs::read_dir(directory)
			.map_err(|error| Problem::io("Inventory metadata directories", error))?
		{
			let entry =
				entry.map_err(|error| Problem::io("Read metadata directory entry", error))?;
			if entry
				.file_type()
				.map_err(|error| Problem::io("Inspect metadata directory", error))?
				.is_dir()
			{
				result.push(
					entry
						.path()
						.strip_prefix(root)
						.expect("state descendant")
						.into(),
				);
				visit(root, &entry.path(), result)?;
			}
		}
		Ok(())
	}
	let mut result = Vec::new();
	visit(state, state, &mut result)?;
	Ok(result)
}
