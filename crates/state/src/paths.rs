use std::path::{Component, Path, PathBuf};
use workyard_core::Problem;

/// Resolve existing ancestors so absent destinations still compare through symlinks.
pub fn canonical_future(path: &Path) -> Result<PathBuf, Problem> {
	let absolute = if path.is_absolute() {
		path.to_path_buf()
	} else {
		std::env::current_dir()
			.map_err(|e| Problem::io("Read current directory", e))?
			.join(path)
	};
	let mut resolved = PathBuf::new();
	for component in absolute.components() {
		match component {
			Component::CurDir => continue,
			Component::ParentDir => {
				resolved.pop();
			}
			other => resolved.push(other),
		}
		match resolved.symlink_metadata() {
			Ok(_) => {
				resolved = resolved
					.canonicalize()
					.map_err(|e| Problem::io(format!("Resolve {resolved:?}"), e))?
			}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
			Err(error) => return Err(Problem::io(format!("Inspect {resolved:?}"), error)),
		}
	}

	Ok(resolved)
}

pub fn exists(path: &Path) -> Result<bool, Problem> {
	match path.symlink_metadata() {
		Ok(_) => Ok(true),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(Problem::io(format!("Inspect {path:?}"), error)),
	}
}
