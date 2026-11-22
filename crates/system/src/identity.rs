#[cfg(windows)]
use std::fs::OpenOptions;
use std::{fs::File, path::Path};
use workyard_core::Problem;

/// Read the volume serial number and file index from the open Windows handle.
#[cfg(windows)]
pub fn file_identity(file: &File) -> Result<(u64, u64), Problem> {
	use std::os::windows::io::AsRawHandle;
	use windows_sys::Win32::Storage::FileSystem::{
		BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
	};

	let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };

	// SAFETY: File owns the valid handle and info is a correctly sized output.
	if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
		return Err(Problem::io(
			"Inspect file identity",
			std::io::Error::last_os_error(),
		));
	}

	Ok((
		info.dwVolumeSerialNumber as u64,
		((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
	))
}

/// Read the native filesystem identity of the selected file.
#[cfg(unix)]
pub fn file_identity(file: &File) -> Result<(u64, u64), Problem> {
	use std::os::unix::fs::MetadataExt;

	let metadata = file
		.metadata()
		.map_err(|e| Problem::io("Inspect file identity", e))?;

	Ok((metadata.dev(), metadata.ino()))
}

/// Identify a named entry without dereferencing a final symlink.
#[cfg(unix)]
pub fn entry_identity(path: &Path) -> Result<(u64, u64), Problem> {
	use std::os::unix::fs::MetadataExt;

	let metadata = path
		.symlink_metadata()
		.map_err(|error| Problem::io("Inspect workspace entry identity", error))?;
	Ok((metadata.dev(), metadata.ino()))
}

/// Identify the directory entry itself, including a reparse point, using its Windows handle.
#[cfg(windows)]
pub fn entry_identity(path: &Path) -> Result<(u64, u64), Problem> {
	use std::os::windows::fs::OpenOptionsExt;
	use windows_sys::Win32::Storage::FileSystem::{
		FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
	};

	let entry = OpenOptions::new()
		.access_mode(FILE_READ_ATTRIBUTES)
		.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
		.open(path)
		.map_err(|error| Problem::io("Open workspace entry identity", error))?;
	file_identity(&entry)
}

/// Read the native filesystem identity of the selected directory.
pub fn directory_identity(path: &Path) -> Result<String, Problem> {
	#[cfg(unix)]
	let directory = File::open(path).map_err(|e| Problem::io("Open repository identity", e))?;

	#[cfg(windows)]
	let directory = {
		use std::os::windows::fs::OpenOptionsExt;
		OpenOptions::new()
			.read(true)
			.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
			.open(path)
			.map_err(|e| Problem::io("Open repository identity", e))?
	};

	let (volume, file) = file_identity(&directory)?;

	Ok(format!("{volume:x}:{file:x}"))
}
