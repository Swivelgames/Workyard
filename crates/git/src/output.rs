//! Decode Git path output without confusing delimiters with path bytes.
use std::{ffi::OsString, path::PathBuf, str::Utf8Error};

pub(crate) fn path_line(bytes: &[u8]) -> Result<PathBuf, Utf8Error> {
	let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
	#[cfg(windows)]
	let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
	path_field(bytes)
}

pub(crate) fn path_field(bytes: &[u8]) -> Result<PathBuf, Utf8Error> {
	#[cfg(unix)]
	{
		use std::os::unix::ffi::OsStringExt;
		Ok(OsString::from_vec(bytes.to_vec()).into())
	}
	#[cfg(windows)]
	{
		Ok(OsString::from(std::str::from_utf8(bytes)?).into())
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn line_delimiter_does_not_remove_newline_from_exact_field() {
		assert_eq!(
			super::path_line(b"repo\n\n").unwrap(),
			std::path::Path::new("repo\n")
		);
		assert_eq!(
			super::path_field(b"repo\n").unwrap(),
			std::path::Path::new("repo\n")
		);
	}

	#[cfg(unix)]
	#[test]
	fn native_non_unicode_bytes_survive_decoding() {
		use std::os::unix::ffi::OsStrExt;
		assert_eq!(
			super::path_field(b"repo\xff")
				.unwrap()
				.as_os_str()
				.as_bytes(),
			b"repo\xff"
		);
	}
}
