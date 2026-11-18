/// Preserve readable text while encoding controls for messages shared across interfaces.
pub(crate) fn escaped(value: &str) -> String {
	value
		.chars()
		.flat_map(|character| {
			if character.is_control() {
				character.escape_default().collect::<Vec<_>>()
			} else {
				vec![character]
			}
		})
		.collect()
}

/// Keep distinct native paths distinguishable when they cannot be rendered as UTF-8.
pub(crate) fn path(path: &std::path::Path) -> String {
	match path.to_str() {
		Some(value) => escaped(value),
		None => format!("{:?}", path.as_os_str()),
	}
}
