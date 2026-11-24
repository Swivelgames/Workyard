//! Workspace-local identity for human command output.

use atomicwrites::{AllowOverwrite, AtomicFile};
use serde::{Deserialize, Serialize};
use std::{
	env, fs,
	io::{self, Write},
	path::Path,
	sync::OnceLock,
};

const MARKER: &str = ".workyard";
static CLI: OnceLock<Identity> = OnceLock::new();

/// Optional settings supplied by an embedding application.
#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct Identity {
	/// Heading text, without an inferred version.
	pub display_name: Option<String>,
	/// Literal command prefix used only in human output.
	pub invocation_prefix: Option<String>,
}

impl Identity {
	fn read(root: &Path) -> io::Result<Self> {
		match fs::read(root.join(MARKER).join("presentation.json")) {
			Ok(bytes) => {
				let settings: Self = serde_json::from_slice(&bytes)?;
				settings.validate()?;
				Ok(settings)
			}
			Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
			Err(error) => Err(error),
		}
	}

	pub(crate) fn validate(&self) -> io::Result<()> {
		for text in [&self.display_name, &self.invocation_prefix]
			.into_iter()
			.flatten()
		{
			if text.trim().is_empty()
				|| text
					.chars()
					.any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}'))
			{
				return Err(io::Error::other(
					"Presentation settings must be nonempty, single-line text.",
				));
			}
		}
		Ok(())
	}

	pub(crate) fn save(self, root: &Path) -> io::Result<()> {
		if self == Self::default() {
			return Ok(());
		}
		let state = root.join(MARKER);
		let guard = fs::OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(false)
			.open(state.join("presentation.lock"))?;
		guard.lock()?;
		let current = Self::read(root)?;
		let updated = Self {
			display_name: self.display_name.or_else(|| current.display_name.clone()),
			invocation_prefix: self
				.invocation_prefix
				.or_else(|| current.invocation_prefix.clone()),
		};
		if updated == current {
			return Ok(());
		}
		AtomicFile::new(state.join("presentation.json"), AllowOverwrite)
			.write(|file| {
				let format = serde_json::ser::PrettyFormatter::with_indent(b"\t");
				updated.serialize(&mut serde_json::Serializer::with_formatter(
					&mut *file, format,
				))?;
				file.write_all(b"\n")
			})
			.map_err(io::Error::from)
	}

	/// Select presentation once for this CLI process. Protocol servers use their native identity.
	pub fn select(workspace: Option<&Path>, overrides: Self) -> io::Result<&'static Self> {
		let cwd = env::current_dir()?;
		let root = workspace.or_else(|| cwd.ancestors().find(|dir| dir.join(MARKER).is_dir()));
		let mut settings = root.map(Self::read).transpose()?.unwrap_or_default();
		// A host can request usage before the first operation prepares local state.
		if let Ok(prefix) = env::var("WORKYARD_CLI_PREFIX") {
			settings.invocation_prefix = Some(prefix);
		}
		settings.display_name = overrides.display_name.or(settings.display_name);
		settings.invocation_prefix = overrides.invocation_prefix.or(settings.invocation_prefix);
		settings.validate()?;
		Ok(CLI.get_or_init(|| settings))
	}

	/// Product name used in human-facing descriptions and diagnostics.
	pub fn name() -> &'static str {
		CLI.get()
			.and_then(|identity| identity.display_name.as_deref())
			.unwrap_or("Workyard")
	}

	/// Visible command prefix for this invocation.
	pub fn prefix() -> &'static str {
		CLI.get()
			.and_then(|settings| settings.invocation_prefix.as_deref())
			.unwrap_or("workyard")
	}

	/// Whether the embedding command selects the workspace for its caller.
	pub fn managed() -> bool {
		CLI.get()
			.is_some_and(|settings| settings.invocation_prefix.is_some())
	}
}
