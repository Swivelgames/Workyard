//! Shell-ready command hints, separate from display-only path abbreviations.
use std::path::Path;

pub(crate) fn argument(value: &str) -> Option<String> {
	if value.chars().any(char::is_control) {
		return None;
	}
	if !value.is_empty()
		&& value
			.chars()
			.all(|character| character.is_ascii_alphanumeric() || "_./-".contains(character))
	{
		return Some(value.into());
	}

	#[cfg(windows)]
	let quoted = format!("'{}'", value.replace('\'', "''"));
	#[cfg(not(windows))]
	let quoted = format!("'{}'", value.replace('\'', "'\"'\"'"));
	Some(quoted)
}

pub(crate) fn path(path: &Path) -> Option<String> {
	let cwd = std::env::current_dir().ok();
	let canonical = cwd.as_ref().and_then(|cwd| cwd.canonicalize().ok());
	let relative = [cwd.as_deref(), canonical.as_deref()]
		.into_iter()
		.flatten()
		.find_map(|base| path.strip_prefix(base).ok());
	let path = relative.unwrap_or(path);
	if path.as_os_str().is_empty() {
		return Some(".".into());
	}
	argument(path.to_str()?)
}

pub(crate) fn workspace_status(state: Option<&Path>) -> Option<String> {
	let mut command = String::from("workyard status");
	if let Some(state) = state {
		command.push_str(&format!(" --state-dir={}", path(state)?));
	}
	Some(command)
}

pub(crate) fn status(task: &str, state: Option<&Path>) -> Option<String> {
	Some(format!(
		"{} -- {}",
		workspace_status(state)?,
		argument(task)?
	))
}

pub(crate) fn recover(task: &str, state: Option<&Path>) -> Option<String> {
	let mut command = String::from("workyard recover");
	if let Some(state) = state {
		command.push_str(&format!(" --state-dir={}", path(state)?));
	}
	if task.starts_with('-') {
		command.push_str(" --");
	}
	Some(format!("{command} {}", argument(task)?))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn simple_arguments_stay_readable_and_controls_are_not_commands() {
		assert_eq!(argument("feat/payments").as_deref(), Some("feat/payments"));
		assert_eq!(argument("state dir").as_deref(), Some("'state dir'"));
		assert!(argument("line\nbreak").is_none());
		assert_eq!(
			status("feat/payments", None).unwrap(),
			"workyard status -- feat/payments"
		);
	}

	#[cfg(unix)]
	#[test]
	fn quoted_arguments_round_trip_through_posix_shell() {
		let value = "state's $(echo unsafe) `echo unsafe` $HOME";
		let command = format!("printf '%s' {}", argument(value).unwrap());
		let output = std::process::Command::new("sh")
			.args(["-c", &command])
			.output()
			.unwrap();
		assert!(output.status.success());
		assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
	}

	#[test]
	fn status_keeps_option_like_paths_as_values() {
		assert_eq!(
			status("feat/example", Some(Path::new("-state"))).unwrap(),
			"workyard status --state-dir=-state -- feat/example"
		);
	}

	#[cfg(unix)]
	#[test]
	fn non_unicode_paths_require_manual_native_quoting() {
		use std::os::unix::ffi::OsStrExt;
		let native = Path::new(std::ffi::OsStr::from_bytes(b"state-\xff"));
		assert!(status("feat/example", Some(native)).is_none());
	}

	#[cfg(windows)]
	#[test]
	fn powershell_literals_double_apostrophes() {
		assert_eq!(argument("state's $HOME").unwrap(), "'state''s $HOME'");
	}
}
