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
