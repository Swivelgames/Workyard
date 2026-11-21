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
	let mut components = absolute.components().peekable();
	while let Some(component) = components.next() {
		match component {
			Component::Prefix(prefix) if components.peek() == Some(&Component::RootDir) => {
				// A drive prefix alone is not the drive root; inspect it with its separator.
				resolved.push(prefix.as_os_str());
				continue;
			}
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

#[cfg(all(test, windows))]
#[test]
fn resolves_existing_and_future_paths_under_a_windows_root() {
	let fixture = tempfile::tempdir().unwrap();
	let canonical = fixture.path().canonicalize().unwrap();
	for directory in [fixture.path(), canonical.as_path()] {
		assert_eq!(canonical_future(directory).unwrap(), canonical);
		let future = directory.join("not created");
		assert_eq!(
			canonical_future(&future).unwrap(),
			canonical.join("not created")
		);
		assert!(!future.exists());
	}
}
