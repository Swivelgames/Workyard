use super::changed;
use std::path::PathBuf;
use workyard_core::Problem;

pub(super) fn registrations(bytes: &[u8]) -> Result<Vec<(PathBuf, String)>, Problem> {
	let mut result = Vec::new();
	let mut path = None;
	for field in bytes.split(|byte| *byte == 0) {
		if let Some(value) = field.strip_prefix(b"worktree ") {
			path = Some(
				crate::output::path_field(value)
					.map_err(|_| changed("Invalid Git path output."))?,
			);
		}
		if let Some(value) = field.strip_prefix(b"branch ") {
			let branch = std::str::from_utf8(value)
				.map_err(|_| changed("Git returned invalid branch metadata."))?;
			if let Some(path) = path.take() {
				result.push((path, branch.into()));
			}
		}
	}
	Ok(result)
}
