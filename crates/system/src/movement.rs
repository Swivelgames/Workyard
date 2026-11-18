//! Native movement never replaces an entry that appeared after preflight.
use std::{io, path::Path};

pub fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
	#[cfg(unix)]
	{
		use std::{ffi::CString, os::unix::ffi::OsStrExt};
		let source = CString::new(source.as_os_str().as_bytes())?;
		let destination = CString::new(destination.as_os_str().as_bytes())?;
		#[cfg(target_os = "linux")]
		// SAFETY: Both C strings are terminated and remain live throughout the call.
		let result = unsafe {
			libc::renameat2(
				libc::AT_FDCWD,
				source.as_ptr(),
				libc::AT_FDCWD,
				destination.as_ptr(),
				libc::RENAME_NOREPLACE,
			)
		};
		#[cfg(target_os = "macos")]
		// SAFETY: Both C strings are terminated and remain live throughout the call.
		let result =
			unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
		#[cfg(not(any(target_os = "linux", target_os = "macos")))]
		return Err(io::Error::new(
			io::ErrorKind::Unsupported,
			"Exclusive native rename is not implemented for this platform.",
		));
		#[cfg(any(target_os = "linux", target_os = "macos"))]
		if result == 0 {
			Ok(())
		} else {
			Err(io::Error::last_os_error())
		}
	}
	#[cfg(windows)]
	{
		use std::os::windows::ffi::OsStrExt;
		let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
		let destination: Vec<_> = destination
			.as_os_str()
			.encode_wide()
			.chain(Some(0))
			.collect();
		if source[..source.len() - 1].contains(&0)
			|| destination[..destination.len() - 1].contains(&0)
		{
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"Path contains NUL.",
			));
		}
		// SAFETY: Both UTF-16 buffers are terminated and live; zero flags forbid replacement.
		let result = unsafe {
			windows_sys::Win32::Storage::FileSystem::MoveFileExW(
				source.as_ptr(),
				destination.as_ptr(),
				0,
			)
		};
		if result != 0 {
			Ok(())
		} else {
			Err(io::Error::last_os_error())
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn existing_destination_is_never_replaced() {
		let directory = tempfile::tempdir().unwrap();
		let source = directory.path().join("source");
		let destination = directory.path().join("destination");
		std::fs::write(&source, "source content").unwrap();
		std::fs::write(&destination, "authored target").unwrap();
		assert!(super::rename_no_replace(&source, &destination).is_err());
		assert_eq!(std::fs::read_to_string(source).unwrap(), "source content");
		assert_eq!(
			std::fs::read_to_string(destination).unwrap(),
			"authored target"
		);
	}
}
