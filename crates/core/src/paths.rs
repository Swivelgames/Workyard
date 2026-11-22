use crate::{Problem, ProblemCode};

/// Check that a branch-shaped task name is safe to use beneath the tasks root.
pub fn validate_task(task: &str) -> Result<(), Problem> {
	if task.is_empty() || task.starts_with('-') || task.split('/').any(|part| !safe_component(part))
	{
		return Err(Problem::new(
			ProblemCode::InvalidTask,
			"Task must be a safe relative branch path.",
		));
	}

	Ok(())
}

/// Check whether a name is safe as one directory component on the current platform.
pub fn safe_component(name: &str) -> bool {
	if name.is_empty()
		|| name == "."
		|| name == ".."
		|| name.chars().any(|c| c.is_control() || "/\\".contains(c))
	{
		return false;
	}

	#[cfg(windows)]
	{
		if name.ends_with('.')
			|| name.ends_with(' ')
			|| name.contains([':', '*', '?', '"', '<', '>', '|'])
		{
			return false;
		}

		let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
		let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL");
		let numbered_device = ["COM", "LPT"].iter().any(|prefix| {
			stem.strip_prefix(prefix).is_some_and(|suffix| {
				matches!(
					suffix,
					"1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
				)
			})
		});
		if device || numbered_device {
			return false;
		}
	}

	true
}

/// Test whether either path contains the other or both denote the same path.
pub fn overlaps(left: &std::path::Path, right: &std::path::Path) -> bool {
	left.starts_with(right) || right.starts_with(left)
}
