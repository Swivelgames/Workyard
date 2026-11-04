use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, io::Write, path::Path};
use workyard_core::Problem;

pub(crate) fn read_versioned<T: DeserializeOwned>(path: &Path) -> Result<T, Problem> {
	let bytes = fs::read(path).map_err(|e| Problem::io(format!("Read {path:?}"), e))?;
	let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
		Problem::new(
			workyard_core::ProblemCode::DamagedState,
			format!("Invalid JSON at {path:?}: {e}"),
		)
	})?;
	if value.get("version").and_then(|v| v.as_u64()) != Some(1) {
		return Err(Problem::new(
			workyard_core::ProblemCode::IncompatibleState,
			format!("Unsupported state version at {path:?}."),
		)
		.remedy("Use a compatible Workyard version; do not delete or reset the state."));
	}

	serde_json::from_value(value).map_err(|e| {
		Problem::new(
			workyard_core::ProblemCode::DamagedState,
			format!("Invalid record at {path:?}: {e}"),
		)
	})
}

pub(crate) fn publish(path: &Path, record: &impl Serialize) -> Result<(), Problem> {
	let mut bytes = Vec::new();
	let format = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut serializer = serde_json::Serializer::with_formatter(&mut bytes, format);
	record.serialize(&mut serializer).map_err(|e| {
		Problem::new(
			workyard_core::ProblemCode::SerializationFailed,
			e.to_string(),
		)
	})?;
	bytes.push(b'\n');

	let result = AtomicFile::new(path, AllowOverwrite).write(|file| {
		file.write_all(&bytes)?;
		file.sync_all()
	});
	if let Err(error) = result {
		// Atomic replacement can succeed before durability reporting fails.
		let visible = fs::read(path).ok().as_deref() == Some(bytes.as_slice());
		let message = format!(
			"Could not confirm durable publication at {path:?} \
			(expected bytes visible: {visible}): {error}"
		);
		let remedy = format!(
			"Inspect {path:?} and the evidence directory alongside it. \
			Preserve existing worktrees; do not rerun create."
		);
		return Err(
			Problem::new(workyard_core::ProblemCode::PublicationUncertain, message).remedy(remedy),
		);
	}

	Ok(())
}
