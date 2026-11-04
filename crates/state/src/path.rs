//! Private lossless path encoding. Plain Unicode paths stay readable.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum StoredPath {
	Text(String),
	Unix { unix: Vec<u8> },
	Windows { windows: Vec<u16> },
}

impl From<&std::path::Path> for StoredPath {
	fn from(path: &std::path::Path) -> Self {
		if let Some(text) = path.to_str() {
			return Self::Text(text.into());
		}
		#[cfg(unix)]
		{
			use std::os::unix::ffi::OsStrExt;
			Self::Unix {
				unix: path.as_os_str().as_bytes().to_vec(),
			}
		}
		#[cfg(windows)]
		{
			use std::os::windows::ffi::OsStrExt;
			Self::Windows {
				windows: path.as_os_str().encode_wide().collect(),
			}
		}
	}
}

impl StoredPath {
	pub fn decode(&self) -> Result<PathBuf, workyard_core::Problem> {
		match self {
			Self::Text(text) => Ok(text.into()),
			#[cfg(unix)]
			Self::Unix { unix } => {
				use std::os::unix::ffi::OsStringExt;
				Ok(std::ffi::OsString::from_vec(unix.clone()).into())
			}
			#[cfg(windows)]
			Self::Windows { windows } => {
				use std::os::windows::ffi::OsStringExt;
				Ok(std::ffi::OsString::from_wide(windows).into())
			}
			_ => Err(workyard_core::Problem::new(
				workyard_core::ProblemCode::IncompatibleState,
				"State contains paths from another OS.",
			)),
		}
	}
}
