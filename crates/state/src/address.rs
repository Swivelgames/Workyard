use sha2::{Digest, Sha256};

pub fn task_filename(name: &str) -> String {
	let mut slug = String::new();
	for character in name.chars() {
		if character.is_ascii_alphanumeric() {
			slug.push(character.to_ascii_lowercase());
		} else if !slug.is_empty() && !slug.ends_with('-') {
			slug.push('-');
		}
	}
	slug.truncate(slug.len().min(48));
	let slug = slug.trim_end_matches('-');
	let slug = if slug.is_empty() { "task" } else { slug };
	let hash: String = Sha256::digest(name.as_bytes())
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect();
	format!("{slug}-{hash}.json")
}
